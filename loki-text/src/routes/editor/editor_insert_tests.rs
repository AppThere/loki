// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::editing::cursor::DocumentPosition;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::{document_to_loro, get_mark_at};

fn doc_with_text(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).expect("document_to_loro")
}

fn pos(byte_offset: usize) -> DocumentPosition {
    DocumentPosition::top_level(0, 0, byte_offset)
}

fn selection(start: usize, end: usize) -> CursorState {
    CursorState {
        loro_cursor: None,
        anchor: Some(pos(start)),
        focus: Some(pos(end)),
        document_generation: 0,
    }
}

fn link_at(loro: &LoroDoc, byte: usize) -> Option<String> {
    match get_mark_at(loro, 0, byte, MARK_LINK_URL).expect("get_mark_at") {
        Some(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

#[test]
fn applies_link_over_selection() {
    let loro = doc_with_text("hello world");
    let applied = set_hyperlink(&loro, &selection(0, 5), "https://example.com").unwrap();
    assert!(applied);
    assert_eq!(link_at(&loro, 0).as_deref(), Some("https://example.com"));
    // Outside the selection there is no link.
    assert_eq!(link_at(&loro, 7), None);
}

#[test]
fn point_cursor_links_word_at_cursor() {
    let loro = doc_with_text("hello world");
    // A point cursor inside "world" links the whole word.
    let applied = set_hyperlink(&loro, &selection(8, 8), "https://w.example").unwrap();
    assert!(applied);
    assert_eq!(link_at(&loro, 8).as_deref(), Some("https://w.example"));
    assert_eq!(link_at(&loro, 0), None);
}

#[test]
fn empty_url_clears_link_and_reports_false() {
    let loro = doc_with_text("hello world");
    assert!(set_hyperlink(&loro, &selection(0, 5), "https://example.com").unwrap());
    let applied = set_hyperlink(&loro, &selection(0, 5), "   ").unwrap();
    assert!(!applied, "blank url clears the link");
    assert_eq!(link_at(&loro, 0), None);
}

#[test]
fn url_is_trimmed() {
    let loro = doc_with_text("hello world");
    set_hyperlink(&loro, &selection(0, 5), "  https://trim.example  ").unwrap();
    assert_eq!(link_at(&loro, 0).as_deref(), Some("https://trim.example"));
}

#[test]
fn applies_link_across_a_multi_paragraph_selection() {
    // Two paragraphs; a selection spanning both must link the tail of the
    // first AND the head of the second (the bug linked only the first).
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![
        Block::Para(vec![Inline::Str("hello".into())]),
        Block::Para(vec![Inline::Str("world".into())]),
    ];
    let loro = document_to_loro(&doc).expect("document_to_loro");
    let cursor = CursorState {
        loro_cursor: None,
        anchor: Some(DocumentPosition::top_level(0, 0, 2)),
        focus: Some(DocumentPosition::top_level(0, 1, 3)),
        document_generation: 0,
    };
    assert!(set_hyperlink(&loro, &cursor, "https://multi.example").unwrap());
    // First paragraph: linked from byte 2 onward.
    let p0 = get_mark_at(&loro, 0, 3, MARK_LINK_URL).expect("get_mark_at p0");
    assert!(
        matches!(p0, Some(LoroValue::String(_))),
        "first para linked"
    );
    // Second paragraph: also linked (byte 1 lies inside 0..3).
    let p1 = get_mark_at(&loro, 1, 1, MARK_LINK_URL).expect("get_mark_at p1");
    assert!(
        matches!(p1, Some(LoroValue::String(_))),
        "second paragraph of the selection must be linked too"
    );
}

/// Encodes a `w`×`h` RGBA PNG into bytes for the image-insert tests.
fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::new(w, h);
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode png");
    bytes
}

#[test]
fn image_inline_carries_data_uri_and_intrinsic_size() {
    let inline = image_inline_from_bytes(&png_bytes(2, 3)).expect("png is supported");
    let Inline::Image(attr, _alt, target) = inline else {
        panic!("expected an image");
    };
    assert!(target.url.starts_with("data:image/png;base64,"));
    let cx = attr
        .kv
        .iter()
        .find(|(k, _)| k == "cx_emu")
        .map(|(_, v)| v.as_str());
    let cy = attr
        .kv
        .iter()
        .find(|(k, _)| k == "cy_emu")
        .map(|(_, v)| v.as_str());
    assert_eq!(cx, Some((2 * EMU_PER_PX_96).to_string().as_str()));
    assert_eq!(cy, Some((3 * EMU_PER_PX_96).to_string().as_str()));
}

#[test]
fn non_image_bytes_are_rejected() {
    assert!(image_inline_from_bytes(b"not an image at all").is_none());
}

#[test]
fn insert_image_at_cursor_places_a_discrete_image() {
    let loro = doc_with_text("ab");
    let image = image_inline_from_bytes(&png_bytes(4, 4)).unwrap();
    let applied = insert_image_at_cursor(&loro, &selection(1, 1), &image).unwrap();
    assert!(applied);
    let doc = loki_doc_model::loro_to_document(&loro).unwrap();
    let Block::Para(inlines) = &doc.sections[0].blocks[0] else {
        panic!("para");
    };
    assert_eq!(inlines.len(), 3, "Str, Image, Str: {inlines:?}");
    assert!(matches!(inlines[1], Inline::Image(..)));
}

#[test]
fn insert_image_without_cursor_is_a_noop() {
    let loro = doc_with_text("ab");
    let image = image_inline_from_bytes(&png_bytes(4, 4)).unwrap();
    let no_cursor = CursorState::new();
    assert!(!insert_image_at_cursor(&loro, &no_cursor, &image).unwrap());
}

// ── Routing into a nested container (a table cell) ──────────────────────

/// A document whose only block (index 0) is a 1×1 table; the cell holds
/// `text`. Returns the live Loro doc.
fn doc_with_table_cell(text: &str) -> LoroDoc {
    use loki_doc_model::content::table::core::{
        Table, TableBody, TableCaption, TableFoot, TableHead,
    };
    use loki_doc_model::content::table::row::{Cell, Row};
    let cell = Cell::simple(vec![Block::Para(vec![Inline::Str(text.into())])]);
    let table = Table {
        attr: Default::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![cell])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    document_to_loro(&doc).expect("document_to_loro")
}

/// A point cursor inside cell 0 (table at block 0) at `byte_offset`.
fn cell_cursor(byte_offset: usize) -> CursorState {
    use loki_doc_model::PathStep;
    let p = DocumentPosition {
        page_index: 0,
        paragraph_index: 0,
        byte_offset,
        path: vec![PathStep::Cell { cell: 0, block: 0 }],
    };
    CursorState {
        loro_cursor: None,
        anchor: Some(p.clone()),
        focus: Some(p),
        document_generation: 0,
    }
}

#[test]
fn hyperlink_routes_into_a_table_cell() {
    let loro = doc_with_table_cell("word");
    // Point cursor inside the cell word → links the whole word in the cell.
    let applied = set_hyperlink(&loro, &cell_cursor(2), "https://cell.example").unwrap();
    assert!(applied);
    let path = loki_doc_model::BlockPath::in_cell(0, 0, 0);
    let mark = loki_doc_model::get_mark_at_path(&loro, &path, 0, MARK_LINK_URL).unwrap();
    assert!(
        matches!(mark, Some(LoroValue::String(ref s)) if s.as_str() == "https://cell.example"),
        "link must land in the cell, got {mark:?}"
    );
}

// ── Table insert places the caret in the first cell (plan 4a.5) ─────────

#[test]
fn first_cell_caret_addresses_top_left_cell() {
    use loki_doc_model::PathStep;
    let p = first_cell_caret(7);
    assert_eq!(p.paragraph_index, 7, "root is the table block index");
    assert_eq!(p.byte_offset, 0);
    assert_eq!(p.path, vec![PathStep::Cell { cell: 0, block: 0 }]);
}

#[test]
fn insert_table_returns_first_cell_caret() {
    // Block 0 is a paragraph; inserting a table after it puts the table at
    // block 1 and returns a caret pointing at that table's first cell.
    let loro = doc_with_text("hello");
    let target = insert_table_after_cursor(&loro, &selection(2, 2))
        .expect("insert ok")
        .expect("a cursor was placed");
    assert_eq!(target, first_cell_caret(1));

    // The table really landed at block 1 and its first cell is an empty,
    // editable paragraph — exactly where the returned caret points.
    let doc = loki_doc_model::loro_to_document(&loro).unwrap();
    let Block::Table(t) = &doc.sections[0].blocks[1] else {
        panic!(
            "block 1 should be the new table: {:?}",
            doc.sections[0].blocks
        );
    };
    let Block::Para(inlines) = &t.bodies[0].body_rows[0].cells[0].blocks[0] else {
        panic!("first cell should hold one empty paragraph");
    };
    assert!(inlines.is_empty(), "first cell paragraph starts empty");
}

#[test]
fn insert_table_without_cursor_is_a_noop() {
    let loro = doc_with_text("hello");
    let target = insert_table_after_cursor(&loro, &CursorState::new()).unwrap();
    assert!(target.is_none(), "no cursor → no table, no caret");
}

#[test]
fn image_routes_into_a_table_cell() {
    let loro = doc_with_table_cell("ab");
    let image = image_inline_from_bytes(&png_bytes(4, 4)).unwrap();
    assert!(insert_image_at_cursor(&loro, &cell_cursor(1), &image).unwrap());
    // Re-derive and confirm the image is a discrete inline in the cell.
    let doc = loki_doc_model::loro_to_document(&loro).unwrap();
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("table");
    };
    let Block::Para(inlines) = &t.bodies[0].body_rows[0].cells[0].blocks[0] else {
        panic!("cell para");
    };
    assert_eq!(inlines.len(), 3, "Str, Image, Str in cell: {inlines:?}");
    assert!(matches!(inlines[1], Inline::Image(..)));
}
