// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regression tests for the Loro bridge data-loss fixes (audit 2026-06-10,
//! findings C1/C2): blocks without a native CRDT mapping must survive a
//! `document_to_loro` → `loro_to_document` round-trip via opaque snapshots,
//! and text-bearing inline variants must keep their text and formatting.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, Caption, ListAttributes};
use loki_doc_model::content::inline::{Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{derive_loro_cursor, document_to_loro, loro_to_document};
use loki_doc_model::loro_schema::{
    BLOCK_TYPE_OPAQUE, BLOCK_TYPE_PARA, KEY_BLOCKS, KEY_SECTIONS, KEY_TYPE,
};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::{CharProps, HighlightColor};

fn round_trip(doc: &Document) -> Document {
    let loro = document_to_loro(doc).expect("document_to_loro must succeed");
    loro_to_document(&loro).expect("loro_to_document must succeed")
}

fn doc_with_blocks(blocks: Vec<Block>) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = blocks;
    doc
}

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.into())])
}

// ── C1: structurally unsupported blocks survive round-trips ─────────────────

#[test]
fn bullet_list_survives_roundtrip() {
    let list = Block::BulletList(vec![
        vec![para("first item")],
        vec![para("second item"), para("continuation")],
    ]);
    let doc = doc_with_blocks(vec![para("before"), list.clone(), para("after")]);
    let recovered = round_trip(&doc);
    assert_eq!(recovered.sections[0].blocks.len(), 3);
    assert_eq!(recovered.sections[0].blocks[1], list);
}

#[test]
fn ordered_list_survives_roundtrip() {
    let list = Block::OrderedList(
        ListAttributes {
            start_number: 4,
            ..Default::default()
        },
        vec![vec![para("item")]],
    );
    let doc = doc_with_blocks(vec![list.clone()]);
    assert_eq!(round_trip(&doc).sections[0].blocks[0], list);
}

#[test]
fn blockquote_survives_roundtrip() {
    let quote = Block::BlockQuote(vec![para("quoted wisdom")]);
    let doc = doc_with_blocks(vec![quote.clone()]);
    assert_eq!(round_trip(&doc).sections[0].blocks[0], quote);
}

#[test]
fn figure_survives_roundtrip() {
    let figure = Block::Figure(
        NodeAttr::default(),
        Caption {
            short: None,
            full: vec![para("a caption")],
        },
        vec![para("figure body")],
    );
    let doc = doc_with_blocks(vec![figure.clone()]);
    assert_eq!(round_trip(&doc).sections[0].blocks[0], figure);
}

// ── C2: paragraphs with non-flattenable inlines survive round-trips ─────────

#[test]
fn footnote_paragraph_survives_roundtrip() {
    let block = Block::Para(vec![
        Inline::Str("body text".into()),
        Inline::Note(NoteKind::Footnote, vec![para("the footnote body")]),
    ]);
    let doc = doc_with_blocks(vec![block.clone()]);
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn inline_image_paragraph_survives_roundtrip() {
    let block = Block::Para(vec![
        Inline::Str("see ".into()),
        Inline::Image(
            NodeAttr::default(),
            vec![Inline::Str("alt text".into())],
            LinkTarget::new("media/image1.png"),
        ),
    ]);
    let doc = doc_with_blocks(vec![block.clone()]);
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

/// Reads the `KEY_TYPE` discriminator of the first block in the first section
/// directly from the Loro document, to distinguish a native mapping from an
/// opaque snapshot.
fn first_block_type(doc: &Document) -> Option<String> {
    let loro = document_to_loro(doc).ok()?;
    let sections = loro.get_list(KEY_SECTIONS);
    let sec = sections.get(0)?.into_container().ok()?.into_map().ok()?;
    let blocks = sec
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    let block = blocks.get(0)?.into_container().ok()?.into_map().ok()?;
    block
        .get(KEY_TYPE)?
        .into_value()
        .ok()?
        .into_string()
        .ok()
        .map(|s| s.to_string())
}

/// A bare (top-level) image is mapped natively — a placeholder anchor carrying
/// the image as a mark — so the paragraph is a live `para`, not an opaque
/// snapshot, while still round-tripping byte-for-byte.
#[test]
fn inline_image_stored_natively_not_opaque() {
    let block = Block::Para(vec![
        Inline::Str("see ".into()),
        Inline::Image(
            NodeAttr::default(),
            vec![Inline::Str("alt".into())],
            LinkTarget::new("media/i.png"),
        ),
        Inline::Str(" here".into()),
    ]);
    let doc = doc_with_blocks(vec![block.clone()]);
    assert_eq!(
        first_block_type(&doc).as_deref(),
        Some(BLOCK_TYPE_PARA),
        "an image paragraph must be a native para, not an opaque snapshot"
    );
    let recovered = round_trip(&doc);
    assert_eq!(recovered.sections[0].blocks[0], block);
    // The image must survive as a discrete, positioned inline (Str, Image, Str).
    let Block::Para(inlines) = &recovered.sections[0].blocks[0] else {
        panic!("expected Para");
    };
    assert_eq!(
        inlines.len(),
        3,
        "image must stay a discrete inline: {inlines:?}"
    );
    assert!(matches!(inlines[1], Inline::Image(..)));
}

/// An image *nested* inside a wrapper is flattened by the text write path, so
/// its block must remain an opaque snapshot to avoid silent data loss.
#[test]
fn nested_inline_image_stays_opaque_but_survives() {
    let block = Block::Para(vec![Inline::Strong(vec![Inline::Image(
        NodeAttr::default(),
        vec![],
        LinkTarget::new("media/i.png"),
    )])]);
    let doc = doc_with_blocks(vec![block.clone()]);
    assert_eq!(
        first_block_type(&doc).as_deref(),
        Some(BLOCK_TYPE_OPAQUE),
        "an image nested in a wrapper must keep its block opaque"
    );
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

// ── C2: text-bearing inline variants keep text and formatting ───────────────

/// Collects all visible text of a block's inlines, descending into runs.
fn visible_text(block: &Block) -> String {
    fn collect(inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Str(s) => out.push_str(s),
                Inline::Space => out.push(' '),
                Inline::StyledRun(run) => collect(&run.content, out),
                Inline::Emph(i) | Inline::Strong(i) | Inline::Underline(i) => collect(i, out),
                _ => {}
            }
        }
    }
    let mut out = String::new();
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => collect(inlines, &mut out),
        _ => {}
    }
    out
}

#[test]
fn strikeout_link_code_text_preserved() {
    let block = Block::Para(vec![
        Inline::Strikeout(vec![Inline::Str("struck".into())]),
        Inline::Space,
        Inline::Link(
            NodeAttr::default(),
            vec![Inline::Str("a link".into())],
            LinkTarget::new("https://example.com"),
        ),
        Inline::Space,
        Inline::Code(NodeAttr::default(), "let x = 1;".into()),
        Inline::Space,
        Inline::Superscript(vec![Inline::Str("up".into())]),
        Inline::SmallCaps(vec![Inline::Str("caps".into())]),
    ]);
    let doc = doc_with_blocks(vec![block]);
    let recovered = round_trip(&doc);
    let text = visible_text(&recovered.sections[0].blocks[0]);
    for expected in ["struck", "a link", "let x = 1;", "up", "caps"] {
        assert!(
            text.contains(expected),
            "lost text {expected:?} in {text:?}"
        );
    }
}

#[test]
fn link_url_preserved_as_hyperlink_prop() {
    let block = Block::Para(vec![Inline::Link(
        NodeAttr::default(),
        vec![Inline::Str("click".into())],
        LinkTarget::new("https://example.com/x"),
    )]);
    let doc = doc_with_blocks(vec![block]);
    let recovered = round_trip(&doc);
    let Block::Para(inlines) = &recovered.sections[0].blocks[0] else {
        panic!("expected Para");
    };
    let hyperlink = inlines.iter().find_map(|i| match i {
        Inline::StyledRun(run) => run.direct_props.as_ref().and_then(|p| p.hyperlink.clone()),
        _ => None,
    });
    assert_eq!(hyperlink.as_deref(), Some("https://example.com/x"));
}

#[test]
fn styled_run_style_id_survives_roundtrip() {
    let block = Block::Para(vec![Inline::StyledRun(StyledRun {
        style_id: Some(StyleId("Emphasis".into())),
        direct_props: None,
        content: vec![Inline::Str("styled".into())],
        attr: NodeAttr::default(),
    })]);
    let doc = doc_with_blocks(vec![block]);
    let recovered = round_trip(&doc);
    let Block::Para(inlines) = &recovered.sections[0].blocks[0] else {
        panic!("expected Para");
    };
    let style_id = inlines.iter().find_map(|i| match i {
        Inline::StyledRun(run) => run.style_id.clone(),
        _ => None,
    });
    assert_eq!(style_id, Some(StyleId("Emphasis".into())));
}

#[test]
fn highlight_none_survives_roundtrip() {
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            highlight_color: Some(HighlightColor::None),
            ..Default::default()
        })),
        content: vec![Inline::Str("plain".into())],
        attr: NodeAttr::default(),
    };
    let doc = doc_with_blocks(vec![Block::Para(vec![Inline::StyledRun(run)])]);
    let recovered = round_trip(&doc);
    let Block::Para(inlines) = &recovered.sections[0].blocks[0] else {
        panic!("expected Para");
    };
    let highlight = inlines.iter().find_map(|i| match i {
        Inline::StyledRun(run) => run.direct_props.as_ref().and_then(|p| p.highlight_color),
        _ => None,
    });
    assert_eq!(highlight, Some(HighlightColor::None));
}

// ── C4: cursor derivation converts UTF-8 byte offsets to Unicode ────────────

#[test]
fn cursor_position_correct_in_non_ascii_text() {
    // "héllo" — 'é' is 2 UTF-8 bytes, so byte offset 3 (start of the first
    // 'l') is Unicode position 2.
    let doc = doc_with_blocks(vec![para("héllo")]);
    let loro = document_to_loro(&doc).expect("document_to_loro must succeed");
    let cursor = derive_loro_cursor(&loro, 0, 3).expect("cursor must resolve");
    let pos = loro
        .get_cursor_pos(&cursor)
        .expect("cursor position must resolve");
    assert_eq!(pos.current.pos, 2);
}

#[test]
fn cursor_at_end_of_non_ascii_text() {
    // "héllo" is 6 UTF-8 bytes but 5 Unicode scalars.
    let doc = doc_with_blocks(vec![para("héllo")]);
    let loro = document_to_loro(&doc).expect("document_to_loro must succeed");
    let cursor = derive_loro_cursor(&loro, 0, 6).expect("cursor must resolve");
    let pos = loro
        .get_cursor_pos(&cursor)
        .expect("cursor position must resolve");
    assert_eq!(pos.current.pos, 5);
}
