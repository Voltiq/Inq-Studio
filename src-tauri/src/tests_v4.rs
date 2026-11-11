//! Tests for the v0.4 engine: undo as differences and appended rendering,
//! protection, optimising, comparing and text export, the structure tree,
//! PDF/A, the invisible text layer of OCR, and signatures.

use crate::edit::{self, FontChoice, LineEdit, NewText};
use crate::history::{self, Render};
use crate::interp::scan_page;
use crate::security::{Allowed, ProtectRequest, Protection};
use crate::{compare, images, ocr, optimize, pages, pdfa, sign, store, tags};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::sync::Arc;

fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

fn page_with(doc: &mut Document, pages_id: ObjectId, font: ObjectId, lines: &[&str]) -> ObjectId {
    let mut ops = vec![op("BT", vec![]), op("Tf", vec!["F1".into(), 12.into()])];
    for (i, l) in lines.iter().enumerate() {
        ops.push(op("Tm", vec![1.into(), 0.into(), 0.into(), 1.into(), 72.into(), (700 - 40 * i as i64).into()]));
        ops.push(op("Tj", vec![Object::string_literal(*l)]));
    }
    ops.push(op("ET", vec![]));
    let content = doc.add_object(Stream::new(dictionary! {}, Content { operations: ops }.encode().unwrap()));
    doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content,
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    })
}

/// A document with one page per entry of `texts`, each a list of lines in Helvetica 12.
fn doc_with(texts: &[&[&str]]) -> Document {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let kids: Vec<Object> = texts.iter().map(|t| Object::Reference(page_with(&mut doc, pages_id, font, t))).collect();
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Count" => kids.len() as i64, "Kids" => kids }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc
}

fn texts(doc: &Document, page: usize) -> Vec<String> {
    scan_page(doc, pages::page_id(doc, page).unwrap()).unwrap().lines.into_iter().map(|l| l.text).collect()
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("inq-v4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn edit_text(doc: &mut Document, page: usize, line: usize, text: &str) {
    let pid = pages::page_id(doc, page).unwrap();
    edit::edit_lines(doc, pid, &[line], &LineEdit { text: Some(text.into()), ..Default::default() }).unwrap();
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

#[test]
fn undo_keeps_only_what_changed() {
    let mut doc = doc_with(&[&["First page"], &["Second page"], &["Third page"]]);
    let before = doc.clone();
    edit_text(&mut doc, 1, 0, "Second page, edited");
    let delta = history::diff(&before, &doc);
    // One page was edited: far fewer objects than the document has.
    assert!(delta.objects.len() <= 3, "{} objects in the step", delta.objects.len());
    assert!(delta.objects.len() < doc.objects.len());
    let after = doc.clone();
    let redo = history::apply(&mut doc, delta);
    assert_eq!(doc.objects, before.objects);
    assert_eq!(texts(&doc, 1), vec!["Second page"]);
    history::apply(&mut doc, redo);
    assert_eq!(doc.objects, after.objects);
    assert_eq!(texts(&doc, 1), vec!["Second page, edited"]);
}

#[test]
fn rendering_bytes_grow_by_appended_updates() {
    for xref_stream in [false, true] {
        let mut doc = doc_with(&[&["First page"], &["Second page"], &["Third page"]]);
        if xref_stream {
            doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceStream;
        }
        let mut render = Render::full(&mut doc, 1).unwrap();
        let full = render.bytes.len();
        let page_ids = pages::page_ids(&doc);

        let before = doc.clone();
        edit_text(&mut doc, 2, 0, "Third page, edited");
        let delta = history::diff(&before, &doc);
        render.append(&doc, &delta.ids()).unwrap();
        assert!(render.bytes.len() - full < full, "the update is smaller than the document");
        assert_eq!(history::changed_pages(&doc, &delta.ids(), &page_ids), Some(vec![2]));
        let read = Document::load_mem(&render.bytes).unwrap();
        history::same_objects(&doc, &read).unwrap();
        assert_eq!(texts(&read, 2), vec!["Third page, edited"]);

        // Undo is one more appended update, and a deleted page changes every page number.
        let step = history::diff(&before, &doc);
        let back = history::apply(&mut doc, step);
        render.append(&doc, &back.ids()).unwrap();
        assert_eq!(texts(&Document::load_mem(&render.bytes).unwrap(), 2), vec!["Third page"]);
        let before = doc.clone();
        pages::delete(&mut doc, &[0]).unwrap();
        let delta = history::diff(&before, &doc);
        assert_eq!(history::changed_pages(&doc, &delta.ids(), &page_ids), None);
        render.append(&doc, &delta.ids()).unwrap();
        let read = Document::load_mem(&render.bytes).unwrap();
        assert_eq!(read.get_pages().len(), 2);
        assert_eq!(texts(&read, 0), vec!["Second page"]);
    }
}

#[test]
fn shared_resources_repaint_every_page_that_uses_them() {
    let mut doc = doc_with(&[&["One"], &["Two"]]);
    let page_ids = pages::page_ids(&doc);
    // The font is shared by both pages.
    let font = doc.objects.iter().find(|(_, o)| matches!(o, Object::Dictionary(d) if d.has(b"BaseFont"))).map(|(id, _)| *id).unwrap();
    doc.get_dictionary_mut(font).unwrap().set("BaseFont", "Courier");
    assert_eq!(history::changed_pages(&doc, &[font], &page_ids), Some(vec![0, 1]));
    assert_eq!(history::changed_pages(&doc, &[], &page_ids), Some(vec![]));
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

#[test]
fn protects_keeps_and_removes_passwords() {
    for aes256 in [true, false] {
        let mut doc = doc_with(&[&["Secret text"]]);
        let req = ProtectRequest {
            open_password: "open".into(),
            owner_password: "owner".into(),
            allowed: Allowed { copy: false, modify: false, ..Default::default() },
            aes256,
        };
        let (bytes, password) = store::full_bytes(&mut doc, &Protection::New(req), &None, true).unwrap();
        assert_eq!(password.as_deref(), Some("owner"));
        // The text is not readable in the file, and the file does not open without a password.
        assert!(!bytes.windows(11).any(|w| w == b"Secret text"));
        assert_eq!(store::load_bytes(&bytes, None).unwrap_err(), "PASSWORD_REQUIRED");
        assert_eq!(store::load_bytes(&bytes, Some("wrong")).unwrap_err(), "PASSWORD_REQUIRED");

        // Opened with the open password alone, and saved again: both passwords still work.
        let opened = store::load_bytes(&bytes, Some("open")).unwrap();
        assert_eq!(texts(&opened, 0), vec!["Secret text"]);
        let state = opened.encryption_state.clone().expect("protection is remembered");
        let info = crate::security::info(&Protection::Keep(state.clone()));
        assert_eq!(info.method, if aes256 { "AES-256" } else { "AES-128" });
        assert!(!info.allowed.copy && !info.allowed.modify && info.allowed.print);
        let mut edited = opened.clone();
        edit_text(&mut edited, 0, 0, "Secret text, edited");
        let (again, _) = store::full_bytes(&mut edited, &Protection::Keep(state), &Some("open".into()), false).unwrap();
        for pw in ["open", "owner"] {
            assert_eq!(texts(&store::load_bytes(&again, Some(pw)).unwrap(), 0), vec!["Secret text, edited"], "{pw}");
        }

        // An incremental save of the protected file encrypts what it appends.
        let base = (Arc::new(bytes.clone()), opened.clone());
        let mut edited = opened.clone();
        edit_text(&mut edited, 0, 0, "Appended secret");
        let inc = store::incremental_bytes(&base, &edited).unwrap();
        assert!(!inc.windows(15).any(|w| w == b"Appended secret"));
        assert_eq!(texts(&store::load_bytes(&inc, Some("open")).unwrap(), 0), vec!["Appended secret"]);

        // Removing the protection writes the file in the clear.
        let mut plain = store::load_bytes(&again, Some("owner")).unwrap();
        let (clear, password) = store::full_bytes(&mut plain, &Protection::None, &None, false).unwrap();
        assert!(password.is_none());
        assert_eq!(texts(&store::load_bytes(&clear, None).unwrap(), 0), vec!["Secret text, edited"]);
    }
    // A request that protects nothing is refused.
    assert!(ProtectRequest { open_password: String::new(), owner_password: "x".into(), allowed: Allowed::default(), aes256: true }.check().is_err());
}

// ---------------------------------------------------------------------------
// Optimise
// ---------------------------------------------------------------------------

fn photo(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let v = ((x * 7 + y * 13) % 251) as u8;
        image::Rgba([v, (x % 256) as u8, (y % 256) as u8, 255])
    })
}

#[test]
fn optimises_images_and_duplicates() {
    let dir = temp_dir("optimize");
    let png = dir.join("photo.png");
    photo(1200, 900).save(&png).unwrap();
    let mut doc = doc_with(&[&["Page one"], &["Page two"]]);
    for i in 0..2 {
        let pid = pages::page_id(&doc, i).unwrap();
        // The same picture on both pages, embedded twice, drawn 4 inches wide: 300 dpi.
        images::insert(&mut doc, pid, &png, Some([72.0, 600.0])).unwrap();
        let index = images::list(&doc, pid).unwrap().len() - 1;
        images::place(&mut doc, pid, index, [72.0, 300.0, 360.0, 516.0]).unwrap();
    }
    let dpi_before = images::list(&doc, pages::page_id(&doc, 0).unwrap()).unwrap()[0].dpi;
    assert!(dpi_before > 290.0, "{dpi_before}");
    let report = optimize::optimize(&mut doc, &optimize::Options { image_dpi: 100.0, jpeg_quality: 70, ..Default::default() }).unwrap();
    assert_eq!(report.images_resampled, 2, "{report:?}");
    assert!(report.duplicates_merged >= 1, "{report:?}");
    assert!(report.size_after * 3 < report.size_before, "{report:?}");
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    let read = Document::load_mem(&buf).unwrap();
    for i in 0..2 {
        let pid = pages::page_id(&read, i).unwrap();
        let im = &images::list(&read, pid).unwrap()[0];
        assert!((im.dpi - 100.0).abs() < 3.0, "{}", im.dpi);
        assert_eq!(im.filter, "DCTDecode");
        // Still where it was, and still decodable.
        assert!((im.bbox[2] - 360.0).abs() < 0.5);
        let scan = scan_page(&read, pid).unwrap();
        assert_eq!(images::decode(&read, scan.images[0].id.unwrap()).unwrap().width(), 400);
    }
    // Both pages now draw one image object.
    let ids: Vec<ObjectId> = (0..2).map(|i| scan_page(&read, pages::page_id(&read, i).unwrap()).unwrap().images[0].id.unwrap()).collect();
    assert_eq!(ids[0], ids[1]);

    // The compact structure reads back the same.
    let (compact, _) = store::full_bytes(&mut doc, &Protection::None, &None, true).unwrap();
    let (plain, _) = store::full_bytes(&mut doc, &Protection::None, &None, false).unwrap();
    assert!(compact.len() < plain.len());
    assert_eq!(texts(&Document::load_mem(&compact).unwrap(), 1), vec!["Page two"]);
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// Compare and export
// ---------------------------------------------------------------------------

#[test]
fn compares_documents_word_by_word() {
    let a = doc_with(&[&["The quick brown fox", "jumps over the lazy dog"], &["An unchanged page"], &["Only in the first"]]);
    let b = doc_with(&[&["A new first page"], &["The quick red fox", "jumps over the lazy dog today"], &["An unchanged page"]]);
    let c = compare::compare(&a, &b).unwrap();
    assert!(!c.same_text);
    // The inserted page pairs with nothing, the removed page with nothing, the others with each other.
    let shape: Vec<(Option<usize>, Option<usize>)> = c.pairs.iter().map(|p| (p.a, p.b)).collect();
    assert!(shape.contains(&(Some(1), Some(2))), "{shape:?}");
    assert_eq!(c.pages_added + c.pages_removed, shape.iter().filter(|(x, y)| x.is_none() || y.is_none()).count());
    let pair = c.pairs.iter().find(|p| p.a == Some(0) && p.b.is_some()).expect("page 1 is paired");
    if pair.b == Some(1) {
        assert_eq!((pair.words_removed, pair.words_added), (1, 2));
        // The mark sits on the word "brown": after "The quick ".
        let q = pair.removed[0];
        assert!(q[0][0] > 72.0 + 40.0 && q[1][0] < 72.0 + 110.0, "{q:?}");
        assert!(c.changes.iter().any(|ch| ch.kind == "replaced" && ch.before == "brown" && ch.after == "red"));
        assert!(c.changes.iter().any(|ch| ch.kind == "added" && ch.after == "today"));
    }
    assert!(compare::compare(&a, &a).unwrap().same_text);
}

#[test]
fn exports_text() {
    let doc = doc_with(&[&["First line", "Second line"], &["Other page"]]);
    let text = compare::document_text(&doc, &[0, 1]).unwrap();
    assert_eq!(text, "First line\n\nSecond line\n\u{c}\nOther page\n");
}

// ---------------------------------------------------------------------------
// Accessibility
// ---------------------------------------------------------------------------

fn tagged_doc() -> Document {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let mc = |tag: &str, id: i64| op("BDC", vec![tag.into(), Object::Dictionary(dictionary! { "MCID" => id })]);
    let ops = vec![
        // Drawn first, read second.
        mc("P", 1),
        op("BT", vec![]),
        op("Tf", vec!["F1".into(), 12.into()]),
        op("Td", vec![72.into(), 600.into()]),
        op("Tj", vec![Object::string_literal("Body text of the page")]),
        op("ET", vec![]),
        op("EMC", vec![]),
        mc("H1", 0),
        op("BT", vec![]),
        op("Tf", vec!["F1".into(), 24.into()]),
        op("Td", vec![72.into(), 700.into()]),
        op("Tj", vec![Object::string_literal("The heading")]),
        op("ET", vec![]),
        op("EMC", vec![]),
        mc("Figure", 2),
        op("re", vec![100.into(), 300.into(), 200.into(), 100.into()]),
        op("f", vec![]),
        op("EMC", vec![]),
    ];
    let content = doc.add_object(Stream::new(dictionary! {}, Content { operations: ops }.encode().unwrap()));
    doc.objects.insert(
        page_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content, "StructParents" => 0,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page_id)] }));
    let root_id = doc.new_object_id();
    let doc_elem = doc.new_object_id();
    let elem = |doc: &mut Document, s: &str, mcid: i64| doc.add_object(dictionary! { "Type" => "StructElem", "S" => s, "P" => doc_elem, "Pg" => page_id, "K" => mcid });
    let h = elem(&mut doc, "Heading1", 0);
    let p = elem(&mut doc, "P", 1);
    let fig = elem(&mut doc, "Figure", 2);
    doc.objects.insert(doc_elem, Object::Dictionary(dictionary! { "Type" => "StructElem", "S" => "Document", "P" => root_id, "K" => vec![h.into(), p.into(), fig.into()] }));
    doc.objects.insert(
        root_id,
        Object::Dictionary(dictionary! { "Type" => "StructTreeRoot", "K" => doc_elem, "RoleMap" => dictionary! { "Heading1" => "H1" } }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id, "StructTreeRoot" => root_id, "MarkInfo" => dictionary! { "Marked" => true } });
    doc.trailer.set("Root", catalog);
    doc
}

#[test]
fn reads_and_edits_the_structure_tree() {
    let mut doc = tagged_doc();
    let info = tags::read(&doc);
    assert!(info.marked && info.has_tree);
    assert_eq!(info.elements, 4);
    assert_eq!((info.figures, info.figures_without_alt), (1, 1));
    let root = &info.nodes[0];
    assert_eq!(root.role, "Document");
    let kids: Vec<(&str, &str)> = root.children.iter().map(|c| (c.role.as_str(), c.text.as_str())).collect();
    assert_eq!(kids, vec![("H1", "The heading"), ("P", "Body text of the page"), ("Figure", "")]);
    assert_eq!(root.children[0].raw, "Heading1");
    assert_eq!(root.children[2].bbox, Some([100.0, 300.0, 300.0, 400.0]));
    assert!(info.issues.iter().any(|i| i.contains("language")));
    assert!(info.issues.iter().any(|i| i.contains("alternative text")));

    // The reading order follows the tags, not the order of drawing.
    let order = tags::reading_order(&doc, 0).unwrap();
    assert_eq!(order.iter().map(|o| o.role.as_str()).collect::<Vec<_>>(), vec!["H1", "P", "Figure"]);
    assert!(order[0].bbox[1] > order[1].bbox[1]);

    let fig = root.children[2].id.clone();
    tags::set_element(&mut doc, &fig, Some("A red rectangle"), None, None).unwrap();
    tags::set_document(&mut doc, Some("en-GB"), Some(true)).unwrap();
    store::info_dict_mut(&mut doc).set("Title", lopdf::text_string("A tagged test"));
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    let info = tags::read(&Document::load_mem(&buf).unwrap());
    assert_eq!(info.figures_without_alt, 0);
    assert_eq!(info.nodes[0].children[2].alt, "A red rectangle");
    assert_eq!(info.lang, "en-GB");
    assert!(info.display_title);
    assert!(info.issues.is_empty(), "{:?}", info.issues);
    assert!(tags::set_document(&mut doc, Some("not a language!"), None).is_err());

    // Without tags, the order is the order of the paragraphs on the page.
    let plain = doc_with(&[&["First", "Second"]]);
    assert!(!tags::read(&plain).has_tree);
    assert_eq!(tags::reading_order(&plain, 0).unwrap().iter().map(|o| o.text.as_str()).collect::<Vec<_>>(), vec!["First", "Second"]);
}

// ---------------------------------------------------------------------------
// PDF/A
// ---------------------------------------------------------------------------

#[test]
fn srgb_profile_is_well_formed() {
    let p = pdfa::srgb_profile();
    assert_eq!(u32::from_be_bytes([p[0], p[1], p[2], p[3]]) as usize, p.len());
    assert_eq!(&p[12..24], b"mntrRGB XYZ ");
    assert_eq!(&p[36..40], b"acsp");
    let n = u32::from_be_bytes([p[128], p[129], p[130], p[131]]) as usize;
    assert_eq!(n, 9);
    for i in 0..n {
        let e = &p[132 + i * 12..144 + i * 12];
        let (off, len) = (u32::from_be_bytes([e[4], e[5], e[6], e[7]]) as usize, u32::from_be_bytes([e[8], e[9], e[10], e[11]]) as usize);
        assert!(off % 4 == 0 && off + len <= p.len(), "tag {}", String::from_utf8_lossy(&e[0..4]));
    }
}

#[test]
fn checks_and_converts_to_pdfa() {
    let mut doc = doc_with(&[&["Archive me"]]);
    let pid = pages::page_id(&doc, 0).unwrap();
    // A script, an annotation that does not print, and a coloured mark.
    let action = doc.add_object(dictionary! { "S" => "JavaScript", "JS" => Object::string_literal("app.alert(1)") });
    let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(root).unwrap().set("OpenAction", action);
    crate::annots::add(&mut doc, pid, &crate::annots::NewAnnot { kind: "rect".into(), rect: Some([100.0, 100.0, 200.0, 150.0]), color: [1.0, 0.0, 0.0], ..Default::default() }).unwrap();
    let annot = crate::annots::annots_array(&doc, pid)[0].as_reference().unwrap();
    doc.get_dictionary_mut(annot).unwrap().set("F", 2);
    store::info_dict_mut(&mut doc).set("Title", lopdf::text_string("Über & <archives>"));
    store::info_dict_mut(&mut doc).set("CreationDate", Object::string_literal("D:20260102030405+01'00'"));

    let level = pdfa::Level { part: 2 };
    let before = pdfa::validate(&doc, level, true);
    let ids: Vec<&str> = before.issues.iter().map(|i| i.id.as_str()).collect();
    for want in ["encrypted", "file-id", "xmp-missing", "font-embed", "output-intent", "actions", "annot-flags"] {
        assert!(ids.contains(&want), "{want} missing from {ids:?}");
    }
    assert!(!before.passed && before.checked > 15);

    let done = pdfa::convert(&mut doc, level).unwrap();
    assert!(done.iter().any(|d| d.contains("sRGB")), "{done:?}");
    // Written out the way the app saves, then read back.
    let (bytes, _) = store::full_bytes(&mut doc, &Protection::None, &None, true).unwrap();
    let read = Document::load_mem(&bytes).unwrap();
    assert_eq!(pdfa::claimed(&read).as_deref(), Some("PDF/A-2b"));
    let after = pdfa::validate(&read, level, false);
    let left: Vec<&str> = after.issues.iter().map(|i| i.id.as_str()).collect();
    // Helvetica comes from Arial where that is installed (every Windows).
    if cfg!(windows) {
        assert!(after.passed, "{:?}", after.issues);
        let f = crate::interp::resource_fonts(&read, &crate::interp::page_resources(&read, pages::page_id(&read, 0).unwrap()), "t");
        assert!(f.values().all(|f| f.program.is_some()));
    } else {
        assert_eq!(left, vec!["font-embed"]);
    }
    assert_eq!(texts(&read, 0), vec!["Archive me"]);
    // The metadata is plain text in the file and agrees with the document information.
    let xmp = String::from_utf8_lossy(&bytes).to_string();
    assert!(xmp.contains("<pdfaid:part>2</pdfaid:part>"));
    assert!(xmp.contains("Über &amp; &lt;archives&gt;"));
    assert!(xmp.contains("<xmp:CreateDate>2026-01-02T03:04:05+01:00</xmp:CreateDate>"));
    // PDF/A-1 is stricter: the same file claims the wrong part.
    assert!(pdfa::validate(&read, pdfa::Level { part: 1 }, false).issues.iter().any(|i| i.id == "xmp-claim"));
    assert!(pdfa::allows_object_streams(&read));
}

// ---------------------------------------------------------------------------
// OCR
// ---------------------------------------------------------------------------

#[test]
fn lays_recognised_words_over_the_page() {
    let mut doc = doc_with(&[&[]]);
    let pid = pages::page_id(&doc, 0).unwrap();
    // A page image at 2 pixels per point: the transform flips y and scales by 2.
    let page = ocr::OcrPage {
        page: 0,
        transform: [2.0, 0.0, 0.0, -2.0, 0.0, 1684.0],
        lines: vec![ocr::OcrLine {
            words: vec![
                ocr::OcrWord { text: "Scanned".into(), x: 200.0, y: 300.0, w: 180.0, h: 40.0 },
                ocr::OcrWord { text: "words".into(), x: 400.0, y: 310.0, w: 120.0, h: 30.0 },
            ],
        }],
    };
    let (n, _) = ocr::add_text_layer(&mut doc, pid, &page, "en").unwrap();
    assert_eq!(n, 2);
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    let read = Document::load_mem(&buf).unwrap();
    let scan = scan_page(&read, pages::page_id(&read, 0).unwrap()).unwrap();
    assert_eq!(scan.lines.len(), 1);
    let l = &scan.lines[0];
    assert_eq!(l.text, "Scanned words");
    assert!(l.invisible);
    // The line starts where the first word was seen and ends where the second one did.
    assert!((l.origin[0] - 100.0).abs() < 0.5, "{:?}", l.origin);
    assert!((l.origin[1] - (842.0 - 340.0 / 2.0 + 0.2 * 20.0)).abs() < 0.6, "{:?}", l.origin);
    assert!((l.origin[0] + l.chars.last().unwrap() - 260.0).abs() < 1.0, "{:?}", l.chars);
    // Text other scripts need comes from an installed font.
    let cjk = ocr::OcrPage { page: 0, transform: page.transform, lines: vec![ocr::OcrLine { words: vec![ocr::OcrWord { text: "Grüße".into(), x: 10.0, y: 10.0, w: 50.0, h: 20.0 }] }] };
    assert_eq!(ocr::add_text_layer(&mut doc, pid, &cjk, "de").unwrap().0, 1);
}

#[cfg(windows)]
#[test]
fn windows_recogniser_answers() {
    // Which language packs are installed differs per computer; an empty page must simply come back empty.
    let Ok(languages) = ocr::languages() else { return };
    if languages.is_empty() {
        return;
    }
    let blank = vec![255u8; 400 * 200];
    let found = ocr::recognize(&blank, 400, 200, None).or_else(|_| ocr::recognize(&blank, 400, 200, Some(&languages[0].tag))).unwrap();
    assert!(found.lines.is_empty());
    assert!(ocr::recognize(&blank, 400, 300, None).is_err());
}

// ---------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------

#[test]
fn leaves_room_for_a_signature_and_fills_it() {
    let mut doc = doc_with(&[&["To be signed"]]);
    let opts = sign::SignOptions { reason: "Approval".into(), location: "Bensheim".into(), page: 0, rect: Some([72.0, 72.0, 272.0, 132.0]), ..Default::default() };
    sign::add_signature(&mut doc, &opts, "Test Signer", "D:20261111081500+01'00'").unwrap();
    let (bytes, _) = store::full_bytes(&mut doc, &Protection::None, &None, false).unwrap();
    let mut seen: Vec<u8> = Vec::new();
    let signed = sign::finish(bytes, |a, b| {
        seen = [a, b].concat();
        Ok(vec![0x30, 0x03, 0x02, 0x01, 0x05])
    })
    .unwrap();
    let read = Document::load_mem(&signed).unwrap();
    let list = sign::list(&read, Some(&signed));
    assert_eq!(list.len(), 1);
    let s = &list[0];
    assert_eq!((s.field.as_str(), s.page, s.reason.as_str(), s.location.as_str()), ("Signature1", Some(0), "Approval", "Bensheim"));
    assert_eq!(s.time, "2026-11-11T08:15:00+01:00");
    assert_eq!(s.rect, Some([72.0, 72.0, 272.0, 132.0]));
    assert!(s.covers_whole);
    // Everything but the value was handed over for signing, and the value sits in the gap.
    assert_eq!(seen.len() + 2 * 16384 + 2, signed.len());
    let hex = String::from_utf8_lossy(&signed).to_string();
    assert!(hex.contains("<3003020105000000"));
    assert!(store::is_signed(&read));
    assert!(sign::empty_fields(&read).is_empty());
}

#[cfg(windows)]
#[test]
fn signs_and_validates_with_a_digital_id() {
    let dir = temp_dir("sign");
    let pfx = dir.join("id.pfx");
    let made = sign::create_identity(&sign::NewIdentity {
        name: "Inq Test Signer".into(),
        email: "signer@example.com".into(),
        organisation: "Inq \"Studio\" Tests".into(),
        country: "de".into(),
        years: 2,
        path: pfx.to_string_lossy().to_string(),
        password: "correct horse".into(),
    })
    .unwrap();
    assert_eq!(made.name, "Inq Test Signer");
    assert!(made.self_signed && !made.expired);
    let path = pfx.to_string_lossy().to_string();
    assert!(sign::file_identity(&path, "wrong").unwrap_err().contains("password"));
    assert_eq!(sign::file_identity(&path, "correct horse").unwrap().thumbprint, made.thumbprint);
    let id = sign::IdentityRef::File { path, password: "correct horse".into() };

    let sign_doc = |doc: &Document, base: Option<&(Arc<Vec<u8>>, Document)>, opts: &sign::SignOptions| -> Vec<u8> {
        sign::sign_with(&id, |who, signer| {
            let mut out = doc.clone();
            sign::add_signature(&mut out, opts, &who.name, &crate::annots::pdf_date())?;
            let bytes = match base {
                Some(b) => store::incremental_bytes(b, &out)?,
                None => store::full_bytes(&mut out, &Protection::None, &None, false)?.0,
            };
            sign::finish(bytes, |a, b| signer(a, b))
        })
        .unwrap()
    };

    let doc = doc_with(&[&["A contract"], &["Second page"]]);
    let first = sign_doc(&doc, None, &sign::SignOptions { reason: "I agree".into(), page: 0, rect: Some([300.0, 72.0, 520.0, 140.0]), ..Default::default() });
    let read = Document::load_mem(&first).unwrap();
    let list = sign::list(&read, Some(&first));
    assert_eq!(list.len(), 1);
    let s = &list[0];
    assert_eq!(s.intact, Some(true), "{}", s.problem);
    assert!(s.covers_whole);
    assert_eq!((s.signer.as_str(), s.digest.as_str(), s.sub_filter.as_str()), ("Inq Test Signer", "SHA-256", "ETSI.CAdES.detached"));
    assert_eq!(s.trust, "untrusted");
    assert!(s.trust_detail.contains("self-signed"), "{}", s.trust_detail);
    assert!(s.subject.contains("signer@example.com") && s.subject.contains("C=DE"), "{}", s.subject);

    // An edit appended afterwards leaves the signature intact, and it no longer covers the whole file.
    let base = (Arc::new(first.clone()), read.clone());
    let mut edited = read.clone();
    let second_page = pages::page_id(&edited, 1).unwrap();
    crate::edit::add_text(&mut edited, second_page, &NewText { x: 72.0, y: 500.0, text: "Added later".into(), font: FontChoice::Standard { name: "Helvetica".into() }, size: 12.0, color: [0.0; 3], line_height: 1.2 }).unwrap();
    let appended = store::incremental_bytes(&base, &edited).unwrap();
    let list = sign::list(&Document::load_mem(&appended).unwrap(), Some(&appended));
    assert_eq!((list[0].intact, list[0].covers_whole), (Some(true), false));
    assert_eq!((list[0].revision, list[0].revisions), (1, 2));

    // A second signature is appended, and both hold.
    let second = sign_doc(&read, Some(&base), &sign::SignOptions { reason: "Countersigned".into(), page: 1, ..Default::default() });
    let list = sign::list(&Document::load_mem(&second).unwrap(), Some(&second));
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|s| s.intact == Some(true)), "{:?}", list.iter().map(|s| &s.problem).collect::<Vec<_>>());
    assert_eq!(list.iter().filter(|s| s.covers_whole).count(), 1);
    assert_eq!(list.iter().map(|s| s.field.as_str()).collect::<Vec<_>>(), vec!["Signature1", "Signature2"]);

    // One changed byte in the signed content is caught.
    let mut tampered = first.clone();
    tampered[120] ^= 0x01;
    let list = sign::list(&read, Some(&tampered));
    assert_eq!(list[0].intact, Some(false));
    let _ = std::fs::remove_dir_all(dir);
}
