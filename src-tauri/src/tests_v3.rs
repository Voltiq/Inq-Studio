//! Tests for the v0.3 engine: shapes and annotation editing, flattening,
//! forms, images, true redaction, page marks, labels, bookmarks and page tools.

use crate::annots::{self, AnnotChange, NewAnnot};
use crate::edit::FontChoice;
use crate::interp::scan_page;
use crate::marks::{LabelRange, Mark, TextStyle};
use crate::outline::Bookmark;
use crate::{forms, images, marks, outline, pages, redact};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

fn reload(doc: &mut Document) -> Document {
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    Document::load_mem(&buf).unwrap()
}

fn first_page(doc: &Document) -> ObjectId {
    pages::page_id(doc, 0).unwrap()
}

/// One page: "Hello World" in Helvetica 24 at (72, 700), a filled square path
/// at (300, 300)-(340, 340), and optional extra resources.
fn base(pages_n: usize) -> Document {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let mut kids = Vec::new();
    for i in 0..pages_n {
        let ops = vec![
            op("BT", vec![]),
            op("Tf", vec!["F1".into(), 24.into()]),
            op("Td", vec![72.into(), 700.into()]),
            op("Tj", vec![Object::string_literal(format!("Hello World {}", i + 1))]),
            op("ET", vec![]),
            op("q", vec![]),
            op("rg", vec![1.into(), 0.into(), 0.into()]),
            op("re", vec![300.into(), 300.into(), 40.into(), 40.into()]),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let content = doc.add_object(Stream::new(dictionary! {}, Content { operations: ops }.encode().unwrap()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => pages_n as i64 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc
}

fn texts(doc: &Document, page: usize) -> Vec<String> {
    scan_page(doc, pages::page_id(doc, page).unwrap()).unwrap().lines.into_iter().map(|l| l.text).collect()
}

fn ann(kind: &str) -> NewAnnot {
    NewAnnot { kind: kind.into(), color: [0.9, 0.1, 0.1], ..Default::default() }
}

#[test]
fn draws_every_annotation_kind() {
    let mut doc = base(1);
    let pid = first_page(&doc);
    let quad = scan_page(&doc, pid).unwrap().lines[0].quad;
    let cases: Vec<NewAnnot> = vec![
        NewAnnot { quads: vec![quad], ..ann("highlight") },
        NewAnnot { quads: vec![quad], ..ann("underline") },
        NewAnnot { quads: vec![quad], ..ann("strikeout") },
        NewAnnot { rect: Some([50.0, 600.0, 50.0, 600.0]), contents: "Note".into(), ..ann("note") },
        NewAnnot { paths: vec![vec![[10.0, 10.0], [50.0, 60.0]]], ..ann("ink") },
        NewAnnot { rect: Some([100.0, 100.0, 200.0, 150.0]), fill: Some([1.0, 1.0, 0.0]), ..ann("rect") },
        NewAnnot { rect: Some([100.0, 200.0, 200.0, 260.0]), ..ann("ellipse") },
        NewAnnot { points: vec![[100.0, 400.0], [200.0, 450.0]], ..ann("line") },
        NewAnnot { points: vec![[100.0, 480.0], [200.0, 500.0]], width: Some(2.0), ..ann("arrow") },
        NewAnnot { points: vec![[300.0, 100.0], [380.0, 120.0], [340.0, 180.0]], ..ann("polygon") },
        NewAnnot { rect: Some([300.0, 500.0, 500.0, 560.0]), contents: "A text box that wraps onto a second line".into(), size: Some(12.0), ..ann("textbox") },
        NewAnnot { rect: Some([350.0, 600.0, 520.0, 650.0]), label: Some("Approved".into()), ..ann("stamp") },
        NewAnnot { rect: Some([72.0, 690.0, 220.0, 725.0]), uri: Some("https://example.com".into()), ..ann("link") },
    ];
    for c in &cases {
        annots::add(&mut doc, pid, c).unwrap_or_else(|e| panic!("{}: {e}", c.kind));
    }
    let doc = reload(&mut doc);
    let list = annots::list(&doc, first_page(&doc));
    let kinds: Vec<&str> = list.iter().map(|a| a.subtype.as_str()).collect();
    assert_eq!(
        kinds,
        ["Highlight", "Underline", "StrikeOut", "Text", "Ink", "Square", "Circle", "Line", "Line", "Polygon", "FreeText", "Stamp", "Link"]
    );
    assert_eq!(list[12].target.as_deref(), Some("https://example.com"));
    // Everything but the link has an appearance stream.
    let arr = annots::annots_array(&doc, first_page(&doc));
    for (i, a) in arr.iter().enumerate().take(12) {
        let d = doc.get_dictionary(a.as_reference().unwrap()).unwrap();
        assert!(d.has(b"AP"), "{} has an appearance", kinds[i]);
    }
}

#[test]
fn edits_and_moves_annotations() {
    let mut doc = base(1);
    let pid = first_page(&doc);
    annots::add(&mut doc, pid, &NewAnnot { rect: Some([100.0, 100.0, 200.0, 150.0]), ..ann("rect") }).unwrap();
    annots::add(&mut doc, pid, &NewAnnot { rect: Some([10.0, 10.0, 60.0, 30.0]), page: Some(0), ..ann("link") }).unwrap();
    annots::update(
        &mut doc,
        pid,
        0,
        &AnnotChange { color: Some([0.0, 0.0, 1.0]), dx: 10.0, dy: -20.0, contents: Some("Moved".into()), opacity: Some(0.5), ..Default::default() },
    )
    .unwrap();
    annots::update(&mut doc, pid, 1, &AnnotChange { uri: Some("https://inq.example".into()), ..Default::default() }).unwrap();
    let list = annots::list(&doc, pid);
    assert_eq!(list[0].rect, [110.0, 80.0, 210.0, 130.0]);
    assert_eq!(list[0].color, Some([0.0, 0.0, 1.0]));
    assert_eq!(list[0].contents, "Moved");
    assert!((list[0].opacity - 0.5).abs() < 1e-4);
    assert_eq!(list[1].target.as_deref(), Some("https://inq.example"));
}

#[test]
fn flattens_annotations_into_the_page() {
    let mut doc = base(1);
    let pid = first_page(&doc);
    annots::add(&mut doc, pid, &NewAnnot { rect: Some([350.0, 600.0, 520.0, 650.0]), label: Some("Final".into()), ..ann("stamp") }).unwrap();
    annots::add(&mut doc, pid, &NewAnnot { rect: Some([10.0, 10.0, 60.0, 30.0]), uri: Some("https://x.test".into()), ..ann("link") }).unwrap();
    let n = forms::flatten_document(&mut doc, false, true).unwrap();
    assert_eq!(n, 1, "links are kept");
    let doc = reload(&mut doc);
    let pid = first_page(&doc);
    assert_eq!(annots::list(&doc, pid).len(), 1);
    let ops = crate::interp::page_content_ops(&doc, pid).unwrap();
    assert!(ops.iter().any(|o| o.operator == "Do"), "the stamp is now page content");
    // The flattened stamp's text is part of the page now.
    assert!(texts(&doc, 0).iter().any(|t| t == "FINAL"), "{:?}", texts(&doc, 0));
}

/// A form with a text field, a check box, a radio group and a combo box.
fn form_doc() -> Document {
    let mut doc = base(1);
    let pid = first_page(&doc);
    let helv = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let mk_state = |doc: &mut Document, w: f32, h: f32| -> ObjectId {
        doc.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), w.into(), h.into()] }, b"0 g 2 2 6 6 re f".to_vec()))
    };
    let on = mk_state(&mut doc, 12.0, 12.0);
    let off = mk_state(&mut doc, 12.0, 12.0);
    let name = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Tx", "T" => Object::string_literal("name"),
        "Rect" => vec![100.into(), 600.into(), 300.into(), 620.into()], "DA" => Object::string_literal("/Helv 0 Tf 0 g"), "P" => pid,
    });
    let agree = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Btn", "T" => Object::string_literal("agree"),
        "Rect" => vec![100.into(), 560.into(), 112.into(), 572.into()], "AS" => "Off", "V" => "Off",
        "AP" => dictionary! { "N" => dictionary! { "Yes" => on, "Off" => off } }, "P" => pid,
    });
    let radio = doc.new_object_id();
    let r1 = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "Parent" => radio, "Rect" => vec![100.into(), 520.into(), 112.into(), 532.into()],
        "AS" => "Off", "AP" => dictionary! { "N" => dictionary! { "Small" => on, "Off" => off } }, "P" => pid,
    });
    let r2 = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "Parent" => radio, "Rect" => vec![130.into(), 520.into(), 142.into(), 532.into()],
        "AS" => "Off", "AP" => dictionary! { "N" => dictionary! { "Large" => on, "Off" => off } }, "P" => pid,
    });
    doc.objects.insert(radio, Object::Dictionary(dictionary! { "FT" => "Btn", "Ff" => 1 << 15, "T" => Object::string_literal("size"), "Kids" => vec![r1.into(), r2.into()] }));
    let combo = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Ch", "Ff" => 1 << 17, "T" => Object::string_literal("color"),
        "Opt" => vec![Object::string_literal("Red"), Object::string_literal("Blue")],
        "Rect" => vec![100.into(), 480.into(), 200.into(), 500.into()], "DA" => Object::string_literal("/Helv 10 Tf 0 g"), "P" => pid,
    });
    doc.get_dictionary_mut(pid).unwrap().set("Annots", vec![name.into(), agree.into(), r1.into(), r2.into(), combo.into()]);
    let form = doc.add_object(dictionary! {
        "Fields" => vec![name.into(), agree.into(), radio.into(), combo.into()],
        "DR" => dictionary! { "Font" => dictionary! { "Helv" => helv } },
        "DA" => Object::string_literal("/Helv 0 Tf 0 g"),
        "NeedAppearances" => true,
    });
    let cat = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(cat).unwrap().set("AcroForm", form);
    doc
}

#[test]
fn fills_in_forms() {
    let mut doc = form_doc();
    let f = forms::fields(&doc);
    let kinds: Vec<(&str, &str)> = f.iter().map(|x| (x.name.as_str(), x.kind.as_str())).collect();
    assert_eq!(kinds, [("name", "text"), ("agree", "checkbox"), ("size", "radio"), ("color", "combo")]);
    assert_eq!(f[2].widgets.len(), 2);
    assert_eq!(f[2].widgets[1].on_state.as_deref(), Some("Large"));
    assert_eq!(f[3].options, ["Red", "Blue"]);

    forms::set_field(&mut doc, "name", "Ada Lovelace – Übung").unwrap();
    forms::set_field(&mut doc, "agree", "Yes").unwrap();
    forms::set_field(&mut doc, "size", "Large").unwrap();
    forms::set_field(&mut doc, "color", "Blue").unwrap();
    let mut doc = reload(&mut doc);
    let f = forms::fields(&doc);
    assert_eq!(f[0].value, "Ada Lovelace – Übung");
    assert!(f[1].widgets[0].checked);
    assert_eq!(f[2].value, "Large");
    assert!(!f[2].widgets[0].checked && f[2].widgets[1].checked);
    assert_eq!(f[3].value, "Blue");

    // Flattening forms puts the filled-in values into the page as text.
    forms::flatten_document(&mut doc, true, false).unwrap();
    let doc = reload(&mut doc);
    assert!(forms::fields(&doc).is_empty());
    let t = texts(&doc, 0);
    assert!(t.iter().any(|x| x == "Ada Lovelace – Übung"), "{t:?}");
    assert!(t.iter().any(|x| x == "Blue"), "{t:?}");
}

fn test_png(dir: &std::path::Path, name: &str, w: u32, h: u32, alpha: bool) -> std::path::PathBuf {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 128, if alpha && x < w / 2 { 100 } else { 255 }]));
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("inq-tests-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn inserts_moves_replaces_extracts_and_deletes_images() {
    let dir = temp_dir("images");
    let png = test_png(&dir, "a.png", 64, 32, true);
    let png2 = test_png(&dir, "b.png", 20, 20, false);
    let mut doc = base(1);
    let pid = first_page(&doc);
    images::insert(&mut doc, pid, &png, Some([100.0, 500.0])).unwrap();
    let list = images::list(&doc, pid).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].width, list[0].height), (64, 32));
    assert!(list[0].has_alpha);
    let b = list[0].bbox;
    assert!((b[0] - 100.0).abs() < 0.01 && (b[3] - 500.0).abs() < 0.01, "{b:?}");
    assert!((b[2] - b[0] - 48.0).abs() < 0.01, "64 px at 96 dpi is 48 pt");

    images::place(&mut doc, pid, 0, [200.0, 200.0, 300.0, 250.0]).unwrap();
    let b = images::list(&doc, pid).unwrap()[0].bbox;
    for (x, y) in b.iter().zip([200.0, 200.0, 300.0, 250.0]) {
        assert!((x - y).abs() < 0.01, "{b:?}");
    }

    images::replace(&mut doc, pid, 0, &png2).unwrap();
    let list = images::list(&doc, pid).unwrap();
    assert_eq!((list[0].width, list[0].height), (20, 20));
    // A square image fitted into a 100 × 50 box: 50 × 50, centred.
    let b = list[0].bbox;
    assert!((b[2] - b[0] - 50.0).abs() < 0.01 && (b[0] - 225.0).abs() < 0.01, "{b:?}");

    let (bytes, ext) = images::extract(&doc, pid, 0).unwrap();
    assert_eq!(ext, "png");
    let back = image::load_from_memory(&bytes).unwrap();
    assert_eq!((back.width(), back.height()), (20, 20));

    images::delete(&mut doc, pid, 0).unwrap();
    assert!(images::list(&doc, pid).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn redacts_text_without_moving_the_rest() {
    let mut doc = base(1);
    let pid = first_page(&doc);
    let before = scan_page(&doc, pid).unwrap();
    let line = &before.lines[0];
    // Cover "World" (characters 6..11).
    let x0 = line.origin[0] + line.chars[6];
    let x1 = line.origin[0] + line.chars[11];
    let r = redact::redact(&mut doc, pid, &[[x0, 690.0, x1, 730.0]], Some([0.0, 0.0, 0.0])).unwrap();
    assert_eq!(r.glyphs, 5);
    let doc = reload(&mut doc);
    let pid = first_page(&doc);
    let after = scan_page(&doc, pid).unwrap();
    let joined: String = after.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("|");
    assert!(!joined.contains("World"), "{joined}");
    assert!(joined.contains("Hello") && joined.contains('1'), "{joined}");
    // The trailing " 1" stays exactly where it was.
    let one = after.runs.iter().find(|r| r.text.contains('1')).unwrap();
    let pos_before = line.origin[0] + line.chars[12];
    let i = one.text.chars().position(|c| c == '1').unwrap();
    let pos_after = one.start.0 + one.chars[i];
    assert!((pos_before - pos_after).abs() < 0.01, "{pos_before} vs {pos_after}");
}

#[test]
fn redacts_images_paths_and_annotations() {
    let dir = temp_dir("redact");
    let png = test_png(&dir, "a.png", 40, 40, false);
    let mut doc = base(1);
    let pid = first_page(&doc);
    images::insert(&mut doc, pid, &png, Some([100.0, 200.0])).unwrap(); // 30 × 30 pt at (100,170)-(130,200)
    annots::add(&mut doc, pid, &NewAnnot { rect: Some([400.0, 400.0, 450.0, 450.0]), ..ann("rect") }).unwrap();
    let r = redact::redact(
        &mut doc,
        pid,
        &[[100.0, 170.0, 115.0, 200.0], [290.0, 290.0, 350.0, 350.0], [390.0, 390.0, 460.0, 460.0]],
        None,
    )
    .unwrap();
    assert_eq!(r.images_changed, 1, "half-covered image is blacked out, not removed");
    assert_eq!(r.paths, 1, "the red square under a box is removed");
    assert_eq!(r.annotations, 1);
    let list = images::list(&doc, pid).unwrap();
    assert_eq!(list.len(), 1);
    let scan = scan_page(&doc, pid).unwrap();
    let id = scan.images[0].id.unwrap();
    let px = images::decode(&doc, id).unwrap();
    assert_eq!(px.get_pixel(2, 20).0, [0, 0, 0, 255], "covered half is black");
    assert_ne!(px.get_pixel(37, 20).0, [0, 0, 0, 255], "the other half is untouched");
    assert!(scan.paths.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn finds_text_for_redaction() {
    let doc = base(3);
    let found = redact::find(&doc, "world").unwrap();
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].quads.len(), 1);
    let q = found[0].quads[0];
    assert!(q[1][0] - q[0][0] > 50.0);
}

fn style() -> TextStyle {
    TextStyle { font: FontChoice::Standard { name: "Helvetica".into() }, size: 10.0, color: [0.2, 0.2, 0.2], opacity: 1.0 }
}

#[test]
fn adds_watermarks_headers_footers_and_bates_numbers() {
    let mut doc = base(3);
    pages::rotate(&mut doc, &[2], 90).unwrap();
    marks::apply(&mut doc, &[0, 1, 2], &Mark::Watermark { text: "DRAFT".into(), style: TextStyle { size: 72.0, opacity: 0.3, ..style() }, angle: 45.0, behind: false }, "report").unwrap();
    marks::apply(
        &mut doc,
        &[0, 1, 2],
        &Mark::HeaderFooter { header: ["{file}".into(), String::new(), "{date}".into()], footer: [String::new(), "Page {page} of {pages}".into(), String::new()], style: style(), margin: 36.0, first_number: 1 },
        "report",
    )
    .unwrap();
    marks::apply(&mut doc, &[0, 1, 2], &Mark::Bates { prefix: "INQ-".into(), suffix: String::new(), start: 41, digits: 6, position: "bottom-right".into(), style: style(), margin: 20.0 }, "report").unwrap();
    let doc = reload(&mut doc);
    for p in 0..3 {
        let t = texts(&doc, p);
        assert!(t.contains(&"DRAFT".to_string()), "{t:?}");
        assert!(t.contains(&format!("Page {} of 3", p + 1)), "{t:?}");
        assert!(t.contains(&format!("INQ-{:06}", 41 + p)), "{t:?}");
        assert!(t.contains(&"report".to_string()));
    }
    // On the rotated page the footer runs upright as shown: along the user y axis.
    let scan = scan_page(&doc, pages::page_id(&doc, 2).unwrap()).unwrap();
    let footer = scan.lines.iter().find(|l| l.text == "Page 3 of 3").unwrap();
    assert!((footer.angle - 90.0).abs() < 0.5, "{}", footer.angle);
    // The page's own text is still there and still first.
    assert_eq!(texts(&doc, 0)[0], "Hello World 1");
    // The watermark is centred: its middle sits on the page centre.
    let scan0 = scan_page(&doc, pages::page_id(&doc, 0).unwrap()).unwrap();
    let wm = scan0.lines.iter().find(|l| l.text == "DRAFT").unwrap();
    let mid = [(wm.quad[0][0] + wm.quad[2][0]) / 2.0, (wm.quad[0][1] + wm.quad[2][1]) / 2.0];
    assert!((mid[0] - 297.5).abs() < 6.0 && (mid[1] - 421.0).abs() < 6.0, "{mid:?}");
}

#[test]
fn sets_page_labels_and_bookmarks() {
    let mut doc = base(5);
    let ranges = vec![
        LabelRange { start: 0, style: "r".into(), prefix: String::new(), first: 1 },
        LabelRange { start: 2, style: "D".into(), prefix: "A-".into(), first: 1 },
    ];
    marks::set_labels(&mut doc, &ranges).unwrap();
    let items = vec![
        Bookmark { title: "Front matter".into(), page: Some(0), top: None, open: true, children: vec![Bookmark { title: "Preface".into(), page: Some(1), top: Some(700.0), open: false, children: vec![] }] },
        Bookmark { title: "Chapter 1 – Über".into(), page: Some(2), top: None, open: false, children: vec![] },
    ];
    outline::set_outline(&mut doc, &items).unwrap();
    let doc = reload(&mut doc);
    assert_eq!(marks::labels(&doc), ranges);
    assert_eq!(marks::label_strings(&marks::labels(&doc), 5), ["i", "ii", "A-1", "A-2", "A-3"]);
    let back = outline::get_outline(&doc);
    assert_eq!(back, items);
}

#[test]
fn crops_resizes_and_splits_pages() {
    let mut doc = base(4);
    let p0 = first_page(&doc);
    annots::add(&mut doc, p0, &NewAnnot { rect: Some([100.0, 100.0, 200.0, 200.0]), ..ann("rect") }).unwrap();
    pages::crop(&mut doc, &[1], Some([50.0, 50.0, 400.0, 500.0])).unwrap();
    let crop = doc.get_dictionary(pages::page_id(&doc, 1).unwrap()).unwrap().get(b"CropBox").unwrap().as_array().unwrap().len();
    assert_eq!(crop, 4);
    // A4 to US Letter: content scaled by min(612/595, 792/842) and centred.
    pages::resize(&mut doc, &[0], 612.0, 792.0).unwrap();
    let pid = first_page(&doc);
    let s = (612.0f32 / 595.0).min(792.0 / 842.0);
    let line = scan_page(&doc, pid).unwrap().lines[0].clone();
    let tx = (612.0 - 595.0 * s) / 2.0;
    assert!((line.origin[0] - (72.0 * s + tx)).abs() < 0.05, "{:?}", line.origin);
    assert!((line.em - 24.0 * s).abs() < 0.01);
    let r = annots::list(&doc, pid)[0].rect;
    assert!((r[0] - (100.0 * s + tx)).abs() < 0.05, "annotations move with the content: {r:?}");
    let parts = pages::split(&doc, &[vec![0, 1], vec![2], vec![3]]).unwrap();
    assert_eq!(parts.iter().map(|d| d.get_pages().len()).collect::<Vec<_>>(), [2, 1, 1]);
    assert_eq!(texts(&parts[1], 0)[0], "Hello World 3");
}

#[allow(dead_code)]
fn _unused(_: Dictionary) {}

/// Writes a v0.3 demo: `INQ_DEMO_OUT=path cargo test make_demo_v3 -- --ignored`
#[test]
#[ignore]
fn make_demo_v3() {
    let Ok(out) = std::env::var("INQ_DEMO_OUT") else { return };
    let mut doc = form_doc();
    forms::set_field(&mut doc, "name", "Ada Lovelace").unwrap();
    forms::set_field(&mut doc, "agree", "Yes").unwrap();
    let pid = first_page(&doc);
    annots::add(&mut doc, pid, &NewAnnot { rect: Some([360.0, 520.0, 530.0, 572.0]), label: Some("Approved".into()), color: [0.1, 0.55, 0.3], ..Default::default() }.with_kind("stamp")).unwrap();
    annots::add(&mut doc, pid, &NewAnnot { points: vec![[300.0, 460.0], [380.0, 500.0]], width: Some(2.0), ..ann("arrow") }).unwrap();
    let dir = temp_dir("demo3");
    let png = test_png(&dir, "logo.png", 120, 60, true);
    images::insert(&mut doc, pid, &png, Some([420.0, 800.0])).unwrap();
    marks::apply(&mut doc, &[0], &Mark::Watermark { text: "SAMPLE".into(), style: TextStyle { size: 90.0, opacity: 0.12, color: [0.4, 0.3, 0.9], ..style() }, angle: 40.0, behind: true }, "demo").unwrap();
    marks::apply(&mut doc, &[0], &Mark::HeaderFooter { header: ["Inq Studio v0.3".into(), String::new(), "{date}".into()], footer: [String::new(), "Page {page} of {pages}".into(), String::new()], style: style(), margin: 30.0, first_number: 1 }, "demo").unwrap();
    doc.compress();
    doc.save(&out).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

trait WithKind {
    fn with_kind(self, k: &str) -> Self;
}

impl WithKind for NewAnnot {
    fn with_kind(mut self, k: &str) -> Self {
        self.kind = k.into();
        self
    }
}
