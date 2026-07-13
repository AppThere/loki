// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 round-trip axis — per-case **P0 fidelity** assertions (audit T-3).
//!
//! Unlike `conformance_round_trip.rs`, which compares two *consecutive*
//! import-export cycles (`a` vs `b`) and is therefore blind to a property that
//! is dropped on the **first** export, each test here builds a model carrying a
//! specific feature, runs a single `export → re-import`, and asserts the
//! feature *survived*. This is the stronger guard the audit asks for — it fails
//! if the exporter silently omits the property (as `w:bidi` and floating-image
//! wrap both did before this pass).
//!
//! - **TC-DOCX-021** — bookmarks + `REF`/`PAGEREF` cross-references.
//! - **TC-DOCX-023** — floating-image text-wrap modes (`wp:anchor`).
//! - **TC-DOCX-029** — right-to-left (`w:bidi`) paragraph direction.

use std::io::Cursor;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget};
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_ooxml::docx::export::DocxExport;
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};

/// A 1×1 transparent PNG as a `data:` URI — the exporter only embeds images
/// whose source is a data URI (`ExportCollector::add_image`).
const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

fn export(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("DOCX export should succeed");
    buf.into_inner()
}

fn import(bytes: Vec<u8>) -> Document {
    DocxImport::import(Cursor::new(bytes), DocxImportOptions::default())
        .expect("DOCX should import")
}

fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    let mut s = Section::new();
    s.blocks = blocks;
    d.sections = vec![s];
    d
}

/// Depth-first walk over every inline in the document (recursing into styled
/// runs), applying `f` and returning `true` on the first match.
fn any_inline(doc: &Document, mut f: impl FnMut(&Inline) -> bool) -> bool {
    fn walk(inlines: &[Inline], f: &mut impl FnMut(&Inline) -> bool) -> bool {
        inlines.iter().any(|i| {
            if f(i) {
                return true;
            }
            match i {
                Inline::StyledRun(r) => walk(&r.content, f),
                Inline::Strong(c)
                | Inline::Emph(c)
                | Inline::Underline(c)
                | Inline::Link(_, c, _)
                | Inline::Span(_, c) => walk(c, f),
                _ => false,
            }
        })
    }
    doc.sections.iter().any(|s| {
        s.blocks.iter().any(|b| {
            let inlines: &[Inline] = match b {
                Block::Para(i) | Block::Plain(i) => i,
                Block::StyledPara(sp) => &sp.inlines,
                Block::Heading(_, _, i) => i,
                _ => return false,
            };
            walk(inlines, &mut f)
        })
    })
}

/// TC-DOCX-021 — a bookmark and a `REF`/`PAGEREF` cross-reference targeting it
/// must both survive an export→re-import.
#[test]
fn tc_docx_021_bookmark_and_cross_reference_round_trip() {
    let seed = doc(vec![
        Block::Para(vec![
            Inline::Bookmark(BookmarkKind::Start, "chapter1".to_string()),
            Inline::Str("Chapter One".to_string()),
            Inline::Bookmark(BookmarkKind::End, "chapter1".to_string()),
        ]),
        Block::Para(vec![
            Inline::Str("See ".to_string()),
            Inline::Field(Field::new(FieldKind::CrossReference {
                target: "chapter1".to_string(),
                format: CrossRefFormat::Page,
            })),
        ]),
    ]);

    let re = import(export(&seed));

    let has_bookmark = any_inline(
        &re,
        |i| matches!(i, Inline::Bookmark(BookmarkKind::Start, name) if name == "chapter1"),
    );
    assert!(
        has_bookmark,
        "bookmark start `chapter1` must survive DOCX round-trip"
    );

    let has_cross_ref = any_inline(&re, |i| {
        matches!(
            i,
            Inline::Field(f)
                if matches!(&f.kind, FieldKind::CrossReference { target, .. } if target == "chapter1")
        )
    });
    assert!(
        has_cross_ref,
        "PAGEREF cross-reference to `chapter1` must survive DOCX round-trip"
    );
}

/// Builds a floating image carrying `wrap`, then returns the `FloatWrap`
/// recovered after one export→re-import (or `None` if the image or its wrap was
/// lost).
fn round_trip_image_wrap(wrap: FloatWrap) -> Option<FloatWrap> {
    let mut attr = NodeAttr::default();
    wrap.store(&mut attr);
    let img = Inline::Image(
        attr,
        vec![Inline::Str("alt".to_string())],
        LinkTarget {
            url: PNG_1X1.to_string(),
            title: None,
        },
    );
    let seed = doc(vec![Block::Para(vec![img])]);
    let re = import(export(&seed));

    let mut found = None;
    any_inline(&re, |i| {
        if let Inline::Image(a, _, _) = i {
            found = FloatWrap::read_or_class_default(a);
            true
        } else {
            false
        }
    });
    found
}

/// TC-DOCX-023 — each floating-image wrap mode (and the behind-text flag) must
/// survive an export→re-import through `wp:anchor`.
#[test]
fn tc_docx_023_floating_image_wrap_modes_round_trip() {
    let cases = [
        FloatWrap {
            wrap: TextWrap::Square,
            side: WrapSide::Both,
            behind_text: false,
        },
        FloatWrap {
            wrap: TextWrap::Tight,
            side: WrapSide::Left,
            behind_text: false,
        },
        FloatWrap {
            wrap: TextWrap::Through,
            side: WrapSide::Right,
            behind_text: false,
        },
        FloatWrap {
            wrap: TextWrap::TopAndBottom,
            side: WrapSide::Both,
            behind_text: false,
        },
        FloatWrap {
            wrap: TextWrap::None,
            side: WrapSide::Both,
            behind_text: true,
        },
    ];

    for expected in cases {
        let got = round_trip_image_wrap(expected)
            .unwrap_or_else(|| panic!("floating image lost for {expected:?}"));
        assert_eq!(
            got.wrap, expected.wrap,
            "wrap mode must survive round-trip for {expected:?}"
        );
        assert_eq!(
            got.behind_text, expected.behind_text,
            "behind-text flag must survive round-trip for {expected:?}"
        );
        // Side is only meaningful for Square/Tight/Through; assert it there.
        if matches!(
            expected.wrap,
            TextWrap::Square | TextWrap::Tight | TextWrap::Through
        ) {
            assert_eq!(
                got.side, expected.side,
                "wrap side must survive round-trip for {expected:?}"
            );
        }
    }
}

/// TC-DOCX-029 — a right-to-left paragraph (`ParaProps.bidi`) must survive an
/// export→re-import (`w:bidi` was imported but dropped on write before this
/// pass).
#[test]
fn tc_docx_029_bidi_paragraph_round_trip() {
    let para = StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            bidi: Some(true),
            ..Default::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str("مرحبا".to_string())],
        attr: NodeAttr::default(),
    };
    let seed = doc(vec![Block::StyledPara(para)]);

    let re = import(export(&seed));

    let bidi = re.sections.iter().find_map(|s| {
        s.blocks.iter().find_map(|b| match b {
            Block::StyledPara(sp) => sp.direct_para_props.as_ref().and_then(|p| p.bidi),
            _ => None,
        })
    });
    assert_eq!(
        bidi,
        Some(true),
        "right-to-left (w:bidi) paragraph direction must survive DOCX round-trip"
    );
}
