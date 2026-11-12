//! Tests for the v0.2 engine: rich lines, paragraphs and reflow, Form
//! XObjects, multi-line and font-change edits, growing subsets, OpenType-CFF
//! embedding, `/Differences` growth and incremental saves.

use crate::edit::{self, FontChoice, LineEdit, NewText};
use crate::interp::scan_page;
use crate::pages;
use crate::sysfonts::{self, LoadedFont};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use std::path::Path;

fn helvetica(doc: &mut Document, base: &str) -> ObjectId {
    doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => Object::Name(base.as_bytes().to_vec()),
        "Encoding" => "WinAnsiEncoding",
    })
}

/// A one-page document with the given content, fonts and form XObjects.
fn page_doc(doc: &mut Document, ops: Vec<Operation>, fonts: Dictionary, xobjects: Dictionary) -> ObjectId {
    let pages_id = doc.new_object_id();
    let content_id = doc.add_object(Stream::new(dictionary! {}, Content { operations: ops }.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
        "Resources" => dictionary! { "Font" => fonts, "XObject" => xobjects },
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    page_id
}

fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

fn s(t: &str) -> Object {
    Object::string_literal(t)
}

fn reload(doc: &mut Document) -> Document {
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    Document::load_mem(&buf).unwrap()
}

fn first_page(doc: &Document) -> ObjectId {
    pages::page_id(doc, 0).unwrap()
}

/// "Hello " regular, "bold" bold, " world" regular, in one BT on one baseline.
fn rich_doc() -> Document {
    let mut doc = Document::with_version("1.7");
    let f1 = helvetica(&mut doc, "Helvetica");
    let f2 = helvetica(&mut doc, "Helvetica-Bold");
    page_doc(
        &mut doc,
        vec![
            op("BT", vec![]),
            op("Tf", vec!["F1".into(), 20.into()]),
            op("Td", vec![72.into(), 700.into()]),
            op("Tj", vec![s("Hello ")]),
            op("Tf", vec!["F2".into(), 20.into()]),
            op("Tj", vec![s("bold")]),
            op("Tf", vec!["F1".into(), 20.into()]),
            op("Tj", vec![s(" world")]),
            op("ET", vec![]),
        ],
        dictionary! { "F1" => f1, "F2" => f2 },
        dictionary! {},
    );
    doc
}

#[test]
fn rich_lines_keep_their_styles() {
    let mut doc = rich_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    assert_eq!(scan.lines.len(), 1);
    let l = &scan.lines[0];
    assert_eq!(l.text, "Hello bold world");
    assert_eq!(l.spans.len(), 3);
    assert_eq!(l.spans[1].font_name, "Helvetica-Bold");
    assert_eq!(l.chars.len(), l.text.chars().count() + 1);
    // Character boundaries grow left to right and end at the line width.
    assert!(l.chars.windows(2).all(|w| w[1] >= w[0] - 1e-3));

    edit::edit_line(&mut doc, pid, 0, &LineEdit { text: Some("Hello very BOLD world!".into()), ..Default::default() }).unwrap();
    let doc = reload(&mut doc);
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    let l = &scan.lines[0];
    assert_eq!(l.text, "Hello very BOLD world!");
    let bold: String = l
        .spans
        .iter()
        .filter(|s| s.font_name == "Helvetica-Bold")
        .map(|s| l.text.chars().skip(s.start).take(s.end - s.start).collect::<String>())
        .collect();
    assert_eq!(bold, "BOLD");
}

/// Four lines of left-aligned 12 pt Helvetica with 14 pt leading.
fn paragraph_doc() -> Document {
    let mut doc = Document::with_version("1.7");
    let f1 = helvetica(&mut doc, "Helvetica");
    let lines = [
        "Inq Studio edits text in the fonts that",
        "the document already uses, and wraps a",
        "paragraph again when its text gets longer",
        "or shorter.",
    ];
    let mut ops = vec![op("BT", vec![]), op("Tf", vec!["F1".into(), 12.into()]), op("TL", vec![14.into()]), op("Td", vec![72.into(), 700.into()])];
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            ops.push(op("T*", vec![]));
        }
        ops.push(op("Tj", vec![s(l)]));
    }
    ops.push(op("ET", vec![]));
    // A separate heading far above, which must not join the paragraph.
    ops.extend([op("BT", vec![]), op("Tf", vec!["F1".into(), 24.into()]), op("Td", vec![72.into(), 780.into()]), op("Tj", vec![s("Title")]), op("ET", vec![])]);
    page_doc(&mut doc, ops, dictionary! { "F1" => f1 }, dictionary! {});
    doc
}

#[test]
fn finds_paragraphs() {
    let doc = paragraph_doc();
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    assert_eq!(scan.lines.len(), 5);
    let para = scan.blocks.iter().find(|b| b.lines.len() == 4).expect("a four-line paragraph");
    assert!((para.leading - 14.0).abs() < 0.01);
    assert_eq!(para.align, "left");
    assert!(para.text.starts_with("Inq Studio edits text in the fonts that the document"));
    assert_eq!(scan.blocks.len(), 2);
}

#[test]
fn reflows_paragraphs() {
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let para = scan.blocks.iter().find(|b| b.lines.len() == 4).unwrap().clone();
    let new_text = format!("{} It now also explains that every line is measured with the font's own advance widths.", para.text);
    let notices = edit::edit_block(&mut doc, pid, para.id, &new_text).unwrap();
    assert!(notices.iter().any(|n| n.contains("lines instead of 4")), "{notices:?}");
    let doc = reload(&mut doc);
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    let para2 = scan.blocks.iter().max_by_key(|b| b.lines.len()).unwrap();
    assert!(para2.lines.len() > 4);
    assert_eq!(para2.text, new_text);
    for &li in &para2.lines {
        let l = &scan.lines[li];
        assert!((l.origin[0] - 72.0).abs() < 0.01, "left edge kept");
        assert!(l.chars.last().unwrap() <= &(para.width + 0.5), "line fits the column: {}", l.text);
    }
    assert!((para2.leading - 14.0).abs() < 0.01);

    // Hard line breaks start new lines.
    let pid = first_page(&doc);
    let mut doc = doc;
    let scan = scan_page(&doc, pid).unwrap();
    let id = scan.blocks.iter().max_by_key(|b| b.lines.len()).unwrap().id;
    edit::edit_block(&mut doc, pid, id, "First paragraph.\nSecond one.").unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let texts: Vec<&str> = scan.lines.iter().map(|l| l.text.as_str()).collect();
    assert!(texts.contains(&"First paragraph.") && texts.contains(&"Second one."), "{texts:?}");
}

/// The lines of the four-line paragraph, top to bottom: text and baseline height.
fn para_lines(doc: &Document) -> Vec<(String, f32, f32)> {
    let scan = scan_page(doc, first_page(doc)).unwrap();
    let mut out: Vec<(String, f32, f32)> = scan.lines.iter().filter(|l| l.em < 20.0).map(|l| (l.text.clone(), l.origin[0], l.origin[1])).collect();
    out.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    out
}

#[test]
fn shortening_a_line_pulls_the_next_words_up() {
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let second = scan.lines.iter().find(|l| l.text.starts_with("the document")).unwrap().id;
    let edit = edit::LineEdit { text: Some("it uses, and wraps a".into()), reflow: true, ..Default::default() };
    edit::edit_lines(&mut doc, pid, &[second], &edit).unwrap();
    let doc = reload(&mut doc);
    let lines = para_lines(&doc);
    assert_eq!(lines[0].0, "Inq Studio edits text in the fonts that", "the line above is untouched");
    assert!(lines[1].0.starts_with("it uses, and wraps a paragraph"), "words moved up: {:?}", lines[1].0);
    let all: Vec<&str> = lines.iter().map(|l| l.0.as_str()).collect();
    assert_eq!(all.join(" "), "Inq Studio edits text in the fonts that it uses, and wraps a paragraph again when its text gets longer or shorter.");
    for (i, l) in lines.iter().enumerate() {
        assert!((l.1 - 72.0).abs() < 0.01, "left edge kept");
        assert!((l.2 - (700.0 - 14.0 * i as f32)).abs() < 0.01, "even leading: {l:?}");
    }

    // Without the option the line changes alone.
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    let edit = edit::LineEdit { text: Some("it uses, and wraps a".into()), ..Default::default() };
    edit::edit_lines(&mut doc, pid, &[second], &edit).unwrap();
    let lines = para_lines(&doc);
    assert_eq!(lines[1].0, "it uses, and wraps a");
    assert_eq!(lines[2].0, "paragraph again when its text gets longer");
}

#[test]
fn a_longer_line_pushes_the_rest_down() {
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let last = scan.lines.iter().find(|l| l.text == "or shorter.").unwrap().id;
    let third = scan.lines.iter().find(|l| l.text.starts_with("paragraph again")).unwrap().id;
    // The third line grows by a few words: they spill into the last line.
    let edit = edit::LineEdit { text: Some("paragraph again, line by line and word by word, when its text gets longer".into()), reflow: true, ..Default::default() };
    edit::edit_lines(&mut doc, pid, &[third], &edit).unwrap();
    let _ = last;
    let doc = reload(&mut doc);
    let lines = para_lines(&doc);
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert_eq!(lines[1].0, "the document already uses, and wraps a");
    assert_eq!(lines[2].0, "paragraph again, line by line and word by");
    assert_eq!(lines[3].0, "word, when its text gets longer or shorter.");
    let all: Vec<&str> = lines.iter().map(|l| l.0.as_str()).collect();
    assert!(all.join(" ").ends_with("when its text gets longer or shorter."), "{all:?}");
    let width = scan.blocks.iter().find(|b| b.lines.len() == 4).unwrap().width;
    let scan2 = scan_page(&doc, first_page(&doc)).unwrap();
    for l in scan2.lines.iter().filter(|l| l.em < 20.0) {
        assert!(l.chars.last().unwrap() <= &(width + 0.5), "fits the column: {}", l.text);
    }
}

#[test]
fn a_new_line_in_the_middle_moves_the_following_lines() {
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let first = scan.lines.iter().find(|l| l.text.starts_with("Inq Studio")).unwrap().id;
    // Ends with a full stop and a short line, so the flow stops at the second line.
    let edit = edit::LineEdit { text: Some("Inq Studio edits text.".into()), reflow: true, ..Default::default() };
    edit::edit_lines(&mut doc, pid, &[first], &edit).unwrap();
    let doc = reload(&mut doc);
    let lines = para_lines(&doc);
    let all: Vec<&str> = lines.iter().map(|l| l.0.as_str()).collect();
    assert_eq!(all.join(" "), "Inq Studio edits text. the document already uses, and wraps a paragraph again when its text gets longer or shorter.");
    for (i, l) in lines.iter().enumerate() {
        assert!((l.2 - (700.0 - 14.0 * i as f32)).abs() < 0.01, "even leading: {l:?}");
    }
}

#[test]
fn deleting_a_line_moves_the_lines_below_up() {
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let second = scan.lines.iter().find(|l| l.text.starts_with("the document")).unwrap().id;
    let edit = edit::LineEdit { delete: true, reflow: true, ..Default::default() };
    edit::edit_lines(&mut doc, pid, &[second], &edit).unwrap();
    let doc = reload(&mut doc);
    let lines = para_lines(&doc);
    let got: Vec<(&str, f32)> = lines.iter().filter(|l| !l.0.is_empty()).map(|l| (l.0.as_str(), l.2)).collect();
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].0, "Inq Studio edits text in the fonts that");
    assert_eq!(got[1].0, "paragraph again when its text gets longer");
    assert!((got[1].1 - 686.0).abs() < 0.01 && (got[2].1 - 672.0).abs() < 0.01, "{got:?}");
}

#[test]
fn justified_paragraphs_stay_justified() {
    let mut doc = Document::with_version("1.7");
    let f1 = helvetica(&mut doc, "Helvetica");
    // Three lines spread to the same right edge with word spacing.
    let words = ["Justified text fills", "the column from edge", "to edge exactly.", "Last"];
    let mut ops = vec![op("BT", vec![]), op("Tf", vec!["F1".into(), 12.into()]), op("TL", vec![15.into()]), op("Td", vec![72.into(), 700.into()])];
    let target = 160.0f32;
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            ops.push(op("T*", vec![]));
        }
        // Natural width from Helvetica metrics, padded with Tw so lines 1-3 end at `target`.
        let natural: f32 = w.chars().map(|c| crate::fonts::tests_width(c)).sum::<f32>() * 12.0 / 1000.0;
        let spaces = w.matches(' ').count() as f32;
        let tw = if i < 3 && spaces > 0.0 { (target - natural) / spaces } else { 0.0 };
        ops.push(op("Tw", vec![Object::Real(tw)]));
        ops.push(op("Tj", vec![s(w)]));
    }
    ops.push(op("ET", vec![]));
    let pid = page_doc(&mut doc, ops, dictionary! { "F1" => f1 }, dictionary! {});
    let scan = scan_page(&doc, pid).unwrap();
    let b = &scan.blocks[0];
    assert_eq!(b.lines.len(), 4);
    assert_eq!(b.align, "justify");
    edit::edit_block(&mut doc, pid, 0, &format!("{} again and again", b.text)).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let full: Vec<f32> = scan.lines.iter().take(scan.lines.len() - 1).map(|l| *l.chars.last().unwrap()).collect();
    for w in &full {
        assert!((w - b.width).abs() < 1.0, "justified line ends at the column edge: {w} vs {}", b.width);
    }
}

#[test]
fn edits_text_inside_form_xobjects() {
    let mut doc = Document::with_version("1.7");
    let f1 = helvetica(&mut doc, "Helvetica");
    let form_ops = Content {
        operations: vec![op("BT", vec![]), op("Tf", vec!["F1".into(), 10.into()]), op("Td", vec![0.into(), 0.into()]), op("Tj", vec![s("Inside a form")]), op("ET", vec![])],
    };
    let form_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 300.into(), 50.into()],
            "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 100.into(), 200.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => f1 } },
        },
        form_ops.encode().unwrap(),
    ));
    let pid = page_doc(
        &mut doc,
        vec![op("q", vec![]), op("Do", vec!["Fm1".into()]), op("Q", vec![])],
        dictionary! {},
        dictionary! { "Fm1" => form_id },
    );
    let scan = scan_page(&doc, pid).unwrap();
    assert_eq!(scan.lines.len(), 1);
    assert_eq!(scan.lines[0].text, "Inside a form");
    assert_eq!(scan.lines[0].origin, [100.0, 200.0], "form matrix applied");
    assert!(scan.lines[0].src.starts_with("form"));
    edit::edit_line(&mut doc, pid, 0, &LineEdit { text: Some("Edited inside the form".into()), ..Default::default() }).unwrap();
    let mut doc = doc;
    let doc = reload(&mut doc);
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    assert_eq!(scan.lines[0].text, "Edited inside the form");
    // The page itself still only draws the form.
    let page_ops = crate::interp::page_content_ops(&doc, pid).unwrap();
    assert!(page_ops.iter().all(|o| o.operator != "Tj"));
}

#[test]
fn edits_several_lines_at_once() {
    let mut doc = paragraph_doc();
    let pid = first_page(&doc);
    edit::edit_lines(&mut doc, pid, &[1, 2], &LineEdit { size: Some(9.0), color: Some([0.8, 0.1, 0.1]), ..Default::default() }).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let by_text = |t: &str| scan.lines.iter().find(|l| l.text.starts_with(t)).unwrap();
    assert!((by_text("the document").em - 9.0).abs() < 1e-3);
    assert!((by_text("paragraph again").em - 9.0).abs() < 1e-3);
    assert!((by_text("Inq Studio").em - 12.0).abs() < 1e-3, "untouched line keeps its size");
    assert_eq!(by_text("the document").color, [0.8, 0.1, 0.1]);
}

#[cfg(windows)]
#[test]
fn changes_the_font_of_a_line() {
    if sysfonts::find_family("Georgia", None, false, false).is_none() {
        return;
    }
    let mut doc = rich_doc();
    let pid = first_page(&doc);
    edit::edit_line(
        &mut doc,
        pid,
        0,
        &LineEdit { font: Some(FontChoice::System { family: "Georgia".into(), bold: false, italic: false }), ..Default::default() },
    )
    .unwrap();
    let doc = reload(&mut doc);
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    assert_eq!(scan.lines[0].text, "Hello bold world");
    assert!(scan.lines[0].spans.iter().all(|s| s.font_name.contains("Georgia")), "{:?}", scan.lines[0].spans.iter().map(|s| &s.font_name).collect::<Vec<_>>());
}

#[cfg(windows)]
#[test]
fn installed_fonts_share_one_growing_subset() {
    if sysfonts::find_family("Arial", None, false, false).is_none() {
        return;
    }
    let mut doc = rich_doc();
    let pid = first_page(&doc);
    for (y, t) in [(500.0, "abc"), (480.0, "xyz ABC"), (460.0, "Mixed: abc xyz €")] {
        edit::add_text(
            &mut doc,
            pid,
            &NewText {
                x: 72.0,
                y,
                text: t.into(),
                font: FontChoice::System { family: "Arial".into(), bold: false, italic: false },
                size: 12.0,
                color: [0.0; 3],
                line_height: 1.2,
            },
        )
        .unwrap();
    }
    let inq_fonts = doc.objects.values().filter(|o| o.as_dict().map(|d| d.has(b"InqSource")).unwrap_or(false)).count();
    assert_eq!(inq_fonts, 1, "one embedded Arial for all three edits");
    let doc = reload(&mut doc);
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    for t in ["abc", "xyz ABC", "Mixed: abc xyz €"] {
        assert!(scan.lines.iter().any(|l| l.text == t), "{t} reads back");
    }
}

fn otf(name: &str) -> Option<LoadedFont> {
    let p = Path::new("C:/Program Files/Epic Games/Launcher/Portal/Content/Font").join(name);
    LoadedFont::from_file(&p, 0)
}

/// OpenType fonts with CFF outlines: the subset is embedded as CIDFontType0
/// with an OpenType program; a subset is reused when it has every glyph.
#[test]
fn embeds_opentype_cff_fonts() {
    let Some(face) = otf("BrutalType-Bold.otf") else {
        eprintln!("no OpenType-CFF test font; skipping");
        return;
    };
    let mut doc = rich_doc();
    let pid = first_page(&doc);
    let emb = sysfonts::embed(&mut doc, &face, "Hello CFF").unwrap();
    let res = edit::add_font_resource(&mut doc, pid, crate::interp::Src::Page, emb.font_id).unwrap();
    let codes: Vec<u8> = "Hello CFF".chars().flat_map(|c| {
        let g = emb.codes[&c];
        [(g >> 8) as u8, g as u8]
    }).collect();
    let mut ops = crate::interp::page_content_ops(&doc, pid).unwrap();
    ops.extend([
        op("BT", vec![]),
        op("Tf", vec![Object::Name(res), 18.into()]),
        op("Td", vec![72.into(), 400.into()]),
        op("Tj", vec![Object::String(codes, StringFormat::Hexadecimal)]),
        op("ET", vec![]),
    ]);
    edit::set_ops(&mut doc, pid, crate::interp::Src::Page, ops).unwrap();
    let mut doc = reload(&mut doc);
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let line = scan.lines.iter().find(|l| l.text == "Hello CFF").expect("reads back through ToUnicode");
    assert_eq!(scan.runs.iter().find(|r| r.text == "Hello CFF").unwrap().font.kind_label(), "CID CFF");
    assert!(line.font_name.contains("BrutalType"));
    // Same glyphs again: reused. New glyphs: a second subset.
    let again = sysfonts::embed(&mut doc, &face, "CFF").unwrap();
    assert_eq!(again.font_id, emb.font_id);
    let more = sysfonts::embed(&mut doc, &face, "Quiz").unwrap();
    assert_ne!(more.font_id, emb.font_id);
}

/// A CID-keyed OpenType-CFF font (Japanese): codes are the glyphs' CIDs.
#[test]
fn embeds_cid_keyed_cff_fonts() {
    let Some(face) = otf("GenEiGothicPro-Bold.otf") else { return };
    let mut doc = rich_doc();
    let emb = sysfonts::embed(&mut doc, &face, "日本語").unwrap();
    assert_eq!(emb.codes.len(), 3);
    let f = crate::fonts::PdfFont::load(&doc, &Object::Reference(emb.font_id), "x".into());
    assert_eq!(f.kind_label(), "CID CFF");
    assert_eq!(f.decode(&[(emb.codes[&'語'] >> 8) as u8, emb.codes[&'語'] as u8])[0].text, "語");
}

/// A simple Type 1 font with a bare CFF program gains a glyph through /Differences.
#[test]
fn grows_simple_cff_fonts_through_differences() {
    let Some(face) = otf("BrutalType-Regular.otf") else { return };
    let parsed = ttf_parser::Face::parse(&face.data, 0).unwrap();
    let cff = parsed.raw_face().table(ttf_parser::Tag::from_bytes(b"CFF ")).unwrap().to_vec();
    let mut doc = Document::with_version("1.7");
    let file = doc.add_object(Stream::new(dictionary! { "Subtype" => "Type1C" }, cff));
    let fd = doc.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "BrutalType-Regular", "Flags" => 32,
        "FontBBox" => vec![0.into(), (-200).into(), 1000.into(), 900.into()], "ItalicAngle" => 0,
        "Ascent" => 900, "Descent" => -200, "CapHeight" => 700, "StemV" => 80, "FontFile3" => file,
    });
    // Only A–F have widths: everything else counts as not drawable yet.
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "BrutalType-Regular", "Encoding" => "WinAnsiEncoding",
        "FirstChar" => 65, "LastChar" => 70,
        "Widths" => vec![600.into(), 600.into(), 600.into(), 600.into(), 600.into(), 600.into()], "FontDescriptor" => fd,
    });
    let pid = page_doc(
        &mut doc,
        vec![op("BT", vec![]), op("Tf", vec!["F1".into(), 20.into()]), op("Td", vec![72.into(), 700.into()]), op("Tj", vec![s("ABC")]), op("ET", vec![])],
        dictionary! { "F1" => font },
        dictionary! {},
    );
    let notice = edit::edit_line(&mut doc, pid, 0, &LineEdit { text: Some("ABCG".into()), ..Default::default() }).unwrap();
    assert!(notice.unwrap_or_default().contains("Added 1 glyph"));
    let doc = reload(&mut doc);
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    assert_eq!(scan.lines[0].text, "ABCG");
    assert_eq!(scan.lines[0].spans.len(), 1, "still one font");
    let enc = doc.get_dictionary(font).unwrap().get(b"Encoding").unwrap();
    assert!(enc.as_dict().unwrap().has(b"Differences"));
}

#[test]
fn saves_incrementally() {
    let mut doc = paragraph_doc();
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();
    let base_doc = Document::load_mem(&original).unwrap();
    let mut doc = Document::load_mem(&original).unwrap();
    let pid = first_page(&doc);
    edit::edit_line(&mut doc, pid, 0, &LineEdit { text: Some("Changed first line".into()), ..Default::default() }).unwrap();
    let bytes = crate::store::incremental_for_tests(&(std::sync::Arc::new(original.clone()), base_doc), &doc).unwrap();
    assert!(bytes.starts_with(&original), "the original file is kept byte for byte");
    assert!(bytes.len() > original.len());
    let tail = String::from_utf8_lossy(&bytes[original.len()..]);
    assert!(tail.contains("/Prev"), "the update points back at the original cross-reference");
    let reread = Document::load_mem(&bytes).unwrap();
    let scan = scan_page(&reread, first_page(&reread)).unwrap();
    assert!(scan.lines.iter().any(|l| l.text == "Changed first line"));
    assert_eq!(reread.revision_count(), 2);
}

/// Paragraph and form statistics for a real file, without printing its text:
/// `INQ_TEST_PDF=path cargo test real_file_v2 -- --ignored --nocapture`
#[test]
#[ignore]
fn real_file_v2() {
    let Ok(path) = std::env::var("INQ_TEST_PDF") else { return };
    let mut doc = Document::load(&path).unwrap();
    let ids = pages::page_ids(&doc);
    let (mut lines, mut blocks, mut multi, mut forms, mut rich) = (0, 0, 0, 0, 0);
    for pid in ids.iter().take(20) {
        let scan = scan_page(&doc, *pid).unwrap();
        lines += scan.lines.len();
        blocks += scan.blocks.len();
        multi += scan.blocks.iter().filter(|b| b.lines.len() > 1).count();
        forms += scan.lines.iter().filter(|l| l.src != "page").count();
        rich += scan.lines.iter().filter(|l| l.spans.len() > 1).count();
    }
    println!("first 20 pages: {lines} lines, {blocks} blocks ({multi} multi-line), {forms} lines in forms, {rich} mixed-style lines");
    // Reflow the biggest paragraph on the first page that has one, with a sentence added.
    for pid in ids.iter().take(20) {
        let scan = scan_page(&doc, *pid).unwrap();
        if let Some(b) = scan.blocks.iter().filter(|b| b.lines.len() > 2).max_by_key(|b| b.lines.len()) {
            let n = b.lines.len();
            let text = format!("{} Added sentence for testing the reflow.", b.text);
            let notices = edit::edit_block(&mut doc, *pid, b.id, &text).unwrap();
            let after = scan_page(&doc, *pid).unwrap();
            let found = after.blocks.iter().any(|x| x.text == text);
            println!("reflowed a {n}-line paragraph: read back exactly {found}; notices {}", notices.len());
            break;
        }
    }
}

/// Two lines far apart: a regular one and a bold, larger one.
fn two_lines_doc() -> Document {
    let mut doc = Document::with_version("1.7");
    let f1 = helvetica(&mut doc, "Helvetica");
    let f2 = helvetica(&mut doc, "Helvetica-Bold");
    let ops = vec![
        op("BT", vec![]),
        op("Tf", vec!["F1".into(), 12.into()]),
        op("Td", vec![72.into(), 700.into()]),
        op("Tj", vec![s("Inq Studio")]),
        op("ET", vec![]),
        op("BT", vec![]),
        op("Tf", vec!["F2".into(), 14.into()]),
        op("Td", vec![100.into(), 500.into()]),
        op("Tj", vec![s("joins lines.")]),
        op("ET", vec![]),
    ];
    page_doc(&mut doc, ops, dictionary! { "F1" => f1, "F2" => f2 }, dictionary! {});
    doc
}

#[test]
fn a_line_dropped_after_another_joins_it() {
    let mut doc = two_lines_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let target = scan.lines.iter().find(|l| l.text == "Inq Studio").unwrap().id;
    let moved = scan.lines.iter().find(|l| l.text == "joins lines.").unwrap().id;
    edit::join_lines(&mut doc, pid, target, moved, false, (0.0, 0.0)).unwrap();
    let doc = reload(&mut doc);
    let scan = scan_page(&doc, first_page(&doc)).unwrap();
    let texts: Vec<&str> = scan.lines.iter().map(|l| l.text.as_str()).filter(|t| !t.is_empty()).collect();
    assert_eq!(texts, vec!["Inq Studio joins lines."]);
    let l = &scan.lines[0];
    assert!((l.origin[0] - 72.0).abs() < 0.01 && (l.origin[1] - 700.0).abs() < 0.01, "stays where the target was: {:?}", l.origin);
    // Each part keeps its own look.
    assert!(l.spans.len() >= 2, "{:?}", l.spans.len());
    let bold = l.spans.iter().find(|sp| sp.bold).expect("the bold part");
    assert!((bold.size - 14.0).abs() < 0.01);
    assert!(l.spans.iter().any(|sp| !sp.bold && (sp.size - 12.0).abs() < 0.01));
}

#[test]
fn a_line_dropped_before_another_joins_in_front() {
    let mut doc = two_lines_doc();
    let pid = first_page(&doc);
    let scan = scan_page(&doc, pid).unwrap();
    let target = scan.lines.iter().find(|l| l.text == "joins lines.").unwrap().id;
    let moved = scan.lines.iter().find(|l| l.text == "Inq Studio").unwrap().id;
    // Dragged down and left to sit in front of the bold line.
    edit::join_lines(&mut doc, pid, target, moved, true, (-40.0, -200.0)).unwrap();
    let scan = scan_page(&doc, pid).unwrap();
    let lines: Vec<_> = scan.lines.iter().filter(|l| !l.text.is_empty()).collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "Inq Studio joins lines.");
    assert!((lines[0].origin[0] - 32.0).abs() < 0.01 && (lines[0].origin[1] - 500.0).abs() < 0.01, "{:?}", lines[0].origin);
    assert!(edit::join_lines(&mut doc, pid, 0, 0, false, (0.0, 0.0)).is_err());
}
