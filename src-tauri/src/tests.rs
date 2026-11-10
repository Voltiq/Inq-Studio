//! End-to-end tests on generated PDFs: scan text, edit it in its own font,
//! save, reload and scan again.

use crate::annots::{self, NewAnnot};
use crate::edit::{self, FontChoice, LineEdit, NewText};
use crate::interp::scan_page;
use crate::pages;
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream, StringFormat};

/// A one-page A4 document with two lines of Helvetica text: one shown with
/// `Tj`, one split over a `TJ` array with kerning, like real producers do.
fn sample() -> Document {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 24.into()]),
            Operation::new("rg", vec![0.2.into(), 0.3.into(), 0.4.into()]),
            Operation::new("Td", vec![72.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal("Hello World")]),
            Operation::new("Td", vec![0.into(), Object::Integer(-40)]),
            Operation::new(
                "TJ",
                vec![Object::Array(vec![
                    Object::string_literal("Inv"),
                    Object::Integer(30),
                    Object::string_literal("oice"),
                    Object::Integer(-300),
                    Object::string_literal("2026"),
                ])],
            ),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    doc
}

fn reload(doc: &mut Document) -> Document {
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    Document::load_mem(&buf).unwrap()
}

fn texts(doc: &Document) -> Vec<String> {
    let pid = pages::page_id(doc, 0).unwrap();
    scan_page(doc, pid).unwrap().lines.into_iter().map(|l| l.text).collect()
}

#[test]
fn scans_lines_with_positions() {
    let doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    assert_eq!(scan.lines.len(), 2);
    let l0 = &scan.lines[0];
    assert_eq!(l0.text, "Hello World");
    assert_eq!(l0.origin, [72.0, 700.0]);
    assert!((l0.em - 24.0).abs() < 1e-3);
    assert_eq!(l0.font_name, "Helvetica");
    // Helvetica "Hello World" is 5.167 em wide (722+556+222+222+556+278+944+556+333+222+556).
    let width = l0.quad[1][0] - l0.quad[0][0];
    assert!((width - 24.0 * 5.167).abs() < 0.5, "width {width}");
    assert!((l0.color[0] - 0.2).abs() < 1e-3);
    // The big negative TJ adjustment reads as a word space.
    assert_eq!(scan.lines[1].text, "Invoice 2026");
    assert!(scan.clean_end);
}

#[test]
fn edits_text_in_the_same_font() {
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    let notice = edit::edit_line(
        &mut doc,
        pid,
        0,
        &LineEdit { text: Some("Grüße, Welt!".into()), ..Default::default() },
    )
    .unwrap();
    assert!(notice.is_none(), "no font change expected: {notice:?}");
    let mut doc = reload(&mut doc);
    assert_eq!(texts(&doc), vec!["Grüße, Welt!", "Invoice 2026"]);
    // Still Helvetica: no new font was added.
    let pid = pages::page_id(&doc, 0).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    assert_eq!(scan.lines[0].font_name, "Helvetica");
    assert_eq!(scan.lines[0].origin, [72.0, 700.0]);

    // Edit the TJ line: all its pieces become one string in the first operator.
    edit::edit_line(&mut doc, pid, 1, &LineEdit { text: Some("Receipt 2027".into()), ..Default::default() }).unwrap();
    let doc = reload(&mut doc);
    assert_eq!(texts(&doc), vec!["Grüße, Welt!", "Receipt 2027"]);
}

#[test]
fn resizes_recolours_moves_and_deletes() {
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    edit::edit_line(
        &mut doc,
        pid,
        0,
        &LineEdit { size: Some(30.0), color: Some([1.0, 0.0, 0.0]), dx: 10.0, dy: -5.0, ..Default::default() },
    )
    .unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let l0 = &scan.lines[0];
    assert_eq!(l0.text, "Hello World");
    assert!((l0.em - 30.0).abs() < 1e-3);
    assert_eq!(l0.color, [1.0, 0.0, 0.0]);
    assert!((l0.origin[0] - 82.0).abs() < 1e-3 && (l0.origin[1] - 695.0).abs() < 1e-3, "{:?}", l0.origin);
    // The next line keeps its place, size and colour.
    let l1 = &scan.lines[1];
    assert_eq!(l1.origin, [72.0, 660.0]);
    assert!((l1.em - 24.0).abs() < 1e-3);
    assert!((l1.color[2] - 0.4).abs() < 1e-3);

    edit::edit_line(&mut doc, pid, 0, &LineEdit { delete: true, ..Default::default() }).unwrap();
    assert_eq!(texts(&doc), vec!["Invoice 2026"]);
}

#[test]
fn adds_text_in_a_document_font() {
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    let key = {
        let scan = scan_page(&doc, pid).unwrap();
        scan.lines[0].font_key.clone()
    };
    edit::add_text(
        &mut doc,
        pid,
        &NewText {
            x: 100.0,
            y: 100.0,
            text: "First line\nSecond line".into(),
            font: FontChoice::Document { key },
            size: 12.0,
            color: [0.0, 0.0, 0.0],
            line_height: 1.2,
        },
    )
    .unwrap();
    let doc = reload(&mut doc);
    let t = texts(&doc);
    assert!(t.contains(&"First line".to_string()) && t.contains(&"Second line".to_string()), "{t:?}");
}

#[test]
fn rejects_characters_no_font_has() {
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    let r = edit::add_text(
        &mut doc,
        pid,
        &NewText {
            x: 10.0,
            y: 10.0,
            text: "\u{10FFFD}".into(),
            font: FontChoice::Standard { name: "Helvetica".into() },
            size: 12.0,
            color: [0.0; 3],
            line_height: 1.2,
        },
    );
    assert!(r.is_err());
}

/// Uses Arial from Windows: embeds a subset, then edits that text with
/// characters the subset does not have, which must embed a new subset of the
/// same typeface rather than fall back to Helvetica.
#[cfg(windows)]
#[test]
fn embeds_and_extends_system_fonts() {
    if crate::sysfonts::find_family("Arial", None, false, false).is_none() {
        eprintln!("Arial is not installed; skipping");
        return;
    }
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    edit::add_text(
        &mut doc,
        pid,
        &NewText {
            x: 72.0,
            y: 500.0,
            text: "Ab €".into(),
            font: FontChoice::System { family: "Arial".into(), bold: false, italic: false },
            size: 14.0,
            color: [0.0; 3],
            line_height: 1.2,
        },
    )
    .unwrap();
    let mut doc = reload(&mut doc);
    let pid = pages::page_id(&doc, 0).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let line = scan.lines.iter().find(|l| l.text == "Ab €").expect("added line");
    assert!(line.font_name.contains("Arial"), "{}", line.font_name);
    let id = line.id;

    let notice = edit::edit_line(&mut doc, pid, id, &LineEdit { text: Some("Quiz Ω".into()), ..Default::default() })
        .unwrap()
        .expect("a notice about the font");
    assert!(notice.contains("Arial"), "{notice}");
    let doc = reload(&mut doc);
    let pid = pages::page_id(&doc, 0).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let line = scan.lines.iter().find(|l| l.text == "Quiz Ω").expect("edited line");
    assert!(line.font_name.contains("Arial"), "{}", line.font_name);
}

#[test]
fn organises_pages() {
    let mut doc = sample();
    pages::insert_blank(&mut doc, 1).unwrap();
    pages::duplicate(&mut doc, 0).unwrap();
    assert_eq!(doc.get_pages().len(), 3);
    // Page order now: original, copy, blank. Move the blank to the front.
    pages::reorder(&mut doc, &[2, 0, 1]).unwrap();
    let doc2 = reload(&mut doc);
    let first = pages::page_id(&doc2, 0).unwrap();
    assert!(scan_page(&doc2, first).unwrap().lines.is_empty());
    let mut doc = doc2;
    pages::rotate(&mut doc, &[0], 90).unwrap();
    pages::rotate(&mut doc, &[0], 270).unwrap();
    pages::rotate(&mut doc, &[0], 90).unwrap();
    assert_eq!(edit::page_rotation(&doc, pages::page_id(&doc, 0).unwrap()), 90);
    pages::delete(&mut doc, &[0]).unwrap();
    assert_eq!(doc.get_pages().len(), 2);
    assert!(pages::delete(&mut doc, &[0, 1]).is_err());

    let extracted = pages::extract(&doc, &[1]).unwrap();
    assert_eq!(extracted.get_pages().len(), 1);

    let mut other = sample();
    let n = pages::insert_document(&mut doc, sample(), 1).unwrap();
    assert_eq!(n, 1);
    let mut doc = reload(&mut doc);
    assert_eq!(doc.get_pages().len(), 3);
    let _ = reload(&mut other);
    assert_eq!(texts(&doc), vec!["Hello World", "Invoice 2026"]);
    let p1 = pages::page_id(&doc, 1).unwrap();
    assert_eq!(scan_page(&doc, p1).unwrap().lines.len(), 2);
    let _ = &mut doc;
}

#[test]
fn adds_and_deletes_annotations() {
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    let quad = scan_page(&doc, pid).unwrap().lines[0].quad;
    annots::add(
        &mut doc,
        pid,
        &NewAnnot {
            kind: "highlight".into(),
            quads: vec![quad],
            rect: None,
            paths: vec![],
            color: [1.0, 0.85, 0.2],
            contents: "Check this".into(),
            author: "Tester".into(),
            width: 1.0,
            opacity: 1.0,
        },
    )
    .unwrap();
    annots::add(
        &mut doc,
        pid,
        &NewAnnot {
            kind: "ink".into(),
            quads: vec![],
            rect: None,
            paths: vec![vec![[10.0, 10.0], [20.0, 30.0], [40.0, 15.0]]],
            color: [0.0, 0.0, 1.0],
            contents: String::new(),
            author: String::new(),
            width: 2.0,
            opacity: 1.0,
        },
    )
    .unwrap();
    let mut doc = reload(&mut doc);
    let pid = pages::page_id(&doc, 0).unwrap();
    let list = annots::list(&doc, pid);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].subtype, "Highlight");
    assert_eq!(list[0].contents, "Check this");
    assert_eq!(list[0].author, "Tester");
    annots::delete(&mut doc, pid, 0).unwrap();
    assert_eq!(annots::list(&doc, pid)[0].subtype, "Ink");
}

#[test]
fn hex_strings_for_two_byte_fonts() {
    // Sanity check that literal strings with parentheses survive encoding.
    let op = Operation::new("Tj", vec![Object::String(b"a(b)c\\".to_vec(), StringFormat::Literal)]);
    let bytes = Content { operations: vec![op] }.encode().unwrap();
    let back = Content::decode(&bytes).unwrap();
    assert_eq!(back.operations[0].operands[0].as_str().unwrap(), b"a(b)c\\");
}

/// Run against a real file without printing its text:
/// `INQ_TEST_PDF=path cargo test real_file -- --ignored --nocapture`
#[test]
#[ignore]
fn real_file() {
    let Ok(path) = std::env::var("INQ_TEST_PDF") else { return };
    let mut doc = Document::load(&path).unwrap();
    let ids = pages::page_ids(&doc);
    println!("pages: {}", ids.len());
    let scan = scan_page(&doc, ids[0]).unwrap();
    let mut fonts: Vec<(String, String)> = scan.runs.iter().map(|r| (r.font.raw_name.clone(), r.font.kind_label().to_string())).collect();
    fonts.sort();
    fonts.dedup();
    println!("page 1: {} lines, {} runs, fonts {:?}", scan.lines.len(), scan.runs.len(), fonts);
    for f in scan.runs.iter().map(|r| r.font.clone()).take(1) {
        println!("first font: subset {}, code_len {}, glyphs {}, extendable {}, to_unicode {}", f.subset, f.code_len, f.reverse.len(), f.extendable, f.has_to_unicode);
    }
    // Rewrite the longest line with its own text plus one character, then reload.
    let Some(line) = scan.lines.iter().filter(|l| l.editable).max_by_key(|l| l.text.len()) else { return };
    let original = line.text.clone();
    let new_text = format!("{original}X");
    let notice = edit::edit_line(&mut doc, ids[0], line.id, &LineEdit { text: Some(new_text.clone()), ..Default::default() }).unwrap();
    println!("edit notice: {}", notice.unwrap_or_else(|| "none (same font)".into()));
    let doc = reload(&mut doc);
    let after = scan_page(&doc, pages::page_ids(&doc)[0]).unwrap();
    let found = after.lines.iter().any(|l| l.text == new_text);
    println!("edited line read back exactly: {found}; lines before {} after {}", scan.lines.len(), after.lines.len());
}

/// Writes a demo document: `INQ_DEMO_OUT=path cargo test make_demo -- --ignored`
#[test]
#[ignore]
fn make_demo() {
    let Ok(out) = std::env::var("INQ_DEMO_OUT") else { return };
    let mut doc = sample();
    let pid = pages::page_id(&doc, 0).unwrap();
    let body = "Inq Studio edits PDF text in the fonts the document already uses.\nThis paragraph is set in Georgia, embedded from this computer as a subset.\nDouble-click any line with the Edit text tool to rewrite it.";
    let fonts = [("Georgia", 15.0, 600.0, body), ("Segoe UI", 11.0, 480.0, "Segoe UI · fully searchable · selectable · 100 % vector")];
    for (family, size, y, text) in fonts {
        let _ = edit::add_text(
            &mut doc,
            pid,
            &NewText {
                x: 72.0,
                y,
                text: text.into(),
                font: FontChoice::System { family: family.into(), bold: false, italic: false },
                size,
                color: [0.1, 0.1, 0.12],
                line_height: 1.45,
            },
        );
    }
    let quad = scan_page(&doc, pid).unwrap().lines[1].quad;
    annots::add(
        &mut doc,
        pid,
        &NewAnnot {
            kind: "highlight".into(),
            quads: vec![quad],
            rect: None,
            paths: vec![],
            color: [1.0, 0.83, 0.23],
            contents: "Generated by the test suite".into(),
            author: "Inq Studio".into(),
            width: 1.0,
            opacity: 1.0,
        },
    )
    .unwrap();
    pages::insert_blank(&mut doc, 1).unwrap();
    doc.compress();
    doc.save(&out).unwrap();
}
