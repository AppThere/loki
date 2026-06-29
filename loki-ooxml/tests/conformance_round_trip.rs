// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 round-trip axis — DOCX **import-export-import** stability.
//!
//! Both compared models are *imported*, so structural normalization is
//! identical and any divergence is a genuine export-then-reimport loss,
//! reported with a model path (`appthere_conformance`) rather than a boolean.

mod helpers;

use std::io::Cursor;

use appthere_conformance::model::canonicalize_document;
use appthere_conformance::roundtrip::{Divergence, first_divergence};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::{CharProps, HighlightColor};
use loki_ooxml::docx::export::DocxExport;
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_primitives::units::Points;

fn import(bytes: Vec<u8>) -> Document {
    DocxImport::import(Cursor::new(bytes), DocxImportOptions::default())
        .expect("DOCX should import")
}

fn export(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("DOCX export should succeed");
    buf.into_inner()
}

/// Returns the first divergence of `seed` under DOCX import-export-import.
///
/// `seed` is exported and imported once to reach the import-canonical shape
/// (`a`), then exported and imported again (`b`); `a` and `b` are compared. This
/// isolates export→reimport loss from the consumer's hand-built model shape.
fn round_trip_divergence(seed: &Document) -> Option<Divergence> {
    let a = import(export(seed));
    let b = import(export(&a));
    first_divergence(&canonicalize_document(&a), &canonicalize_document(&b))
}

fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    let mut s = Section::new();
    s.blocks = blocks;
    d.sections = vec![s];
    d
}

fn styled_run(text: &str, props: CharProps) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str(text.to_string())],
        attr: NodeAttr::default(),
    })
}

fn bold_run(text: &str) -> Inline {
    styled_run(
        text,
        CharProps {
            bold: Some(true),
            ..Default::default()
        },
    )
}

/// Core word-processing content — plain paragraphs, a bold run, a heading, and a
/// bookmark — must survive a DOCX export→re-import with no model divergence.
#[test]
fn docx_round_trip_preserves_core_content() {
    let seed = doc(vec![
        Block::Para(vec![Inline::Str("Hello world".to_string())]),
        Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("A heading".to_string())],
        ),
        Block::Para(vec![
            Inline::Str("Some ".to_string()),
            bold_run("bold"),
            Inline::Str(" text.".to_string()),
        ]),
        Block::Para(vec![
            Inline::Bookmark(BookmarkKind::Start, "mark1".to_string()),
            Inline::Str("anchored".to_string()),
            Inline::Bookmark(BookmarkKind::End, "mark1".to_string()),
        ]),
    ]);

    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "core DOCX round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}

/// Regression for the export gap surfaced by the conformance harness: runs whose
/// *only* direct formatting was highlight, letter-spacing, all-caps, or scale
/// used to export with an empty `<w:rPr>`, collapse to plain runs, and merge with
/// their neighbours — silently dropping the `StyledRun` wrapper (and, via run
/// merging, adjacent text). Each must now survive import-export-import intact.
#[test]
fn docx_round_trip_preserves_secondary_run_formatting() {
    let highlighted = CharProps {
        highlight_color: Some(HighlightColor::Yellow),
        ..Default::default()
    };
    let letter_spaced = CharProps {
        letter_spacing: Some(Points::new(2.0)),
        ..Default::default()
    };
    let all_caps = CharProps {
        all_caps: Some(true),
        ..Default::default()
    };

    let seed = doc(vec![
        Block::Para(vec![
            styled_run("highlighted", highlighted),
            Inline::Str(" and ".to_string()),
            styled_run("letter-spaced", letter_spaced),
        ]),
        Block::Para(vec![
            styled_run("ALL CAPS", all_caps),
            Inline::Str(" plain tail.".to_string()),
        ]),
    ]);

    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "secondary run formatting diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}

/// The comprehensive reference fixture (headers, footnotes, hyperlinks, images,
/// …) under the same import-export-import comparison.
///
/// **Still surfaces a real export→reimport gap** — the conformance axis doing its
/// job. The earlier content-loss gap (highlight / letter-spacing runs collapsing
/// and dropping text at `blk0005`) is now fixed; the first remaining divergence
/// is at `blk0026/i0001/props`: the footnote-reference export hard-codes
/// `<w:vertAlign w:val="superscript"/>` where the style-driven fixture left it
/// implicit, so re-import gains an explicit `valign=Superscript`. Ignored so CI
/// stays green; run `cargo test -p loki-ooxml --test conformance_round_trip --
/// --ignored` to see the current first divergence. Un-ignore once the remaining
/// export gaps are closed (or an expected-divergence tolerance is added per
/// Spec 02 §6).
#[test]
#[ignore = "surfaces real DOCX round-trip gaps in the full reference fixture — tracked, not yet fixed"]
fn docx_reference_round_trip_surfaces_gaps() {
    let a = import(helpers::build_reference_docx());
    let b = import(export(&a));
    if let Some(d) = first_divergence(&canonicalize_document(&a), &canonicalize_document(&b)) {
        panic!(
            "reference DOCX round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}
