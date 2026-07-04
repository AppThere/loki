// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 round-trip axis — ODT **import-export-import** stability.
//!
//! Mirrors the DOCX shape (`loki-ooxml/tests/conformance_round_trip.rs`) for
//! `loki-odf`'s ODT writer + reader. Both compared models are *imported*, so any
//! divergence is a genuine export→re-import loss, reported with a model path by
//! `appthere_conformance` rather than a bespoke per-field assertion.

use std::io::Cursor;

use appthere_conformance::model::canonicalize_document;
use appthere_conformance::roundtrip::{Divergence, first_divergence};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline, StyledRun};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_odf::odt::export::{OdtExport, OdtExportOptions};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};

fn import(bytes: Vec<u8>) -> Document {
    OdtImport::import(Cursor::new(bytes), OdtImportOptions::default()).expect("ODT should import")
}

fn export(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    OdtExport::export(doc, &mut buf, OdtExportOptions::default())
        .expect("ODT export should succeed");
    buf.into_inner()
}

/// First divergence of `seed` under ODT import-export-import (see the DOCX
/// sibling for the rationale of comparing two *imported* models).
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

/// A single-body table whose cells hold the given paragraph texts.
fn table(rows: Vec<Vec<&str>>) -> Block {
    let body_rows = rows
        .into_iter()
        .map(|cells| {
            Row::new(
                cells
                    .into_iter()
                    .map(|t| Cell::simple(vec![Block::Para(vec![Inline::Str(t.to_string())])]))
                    .collect(),
            )
        })
        .collect();
    Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: vec![],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(body_rows)],
        foot: TableFoot::empty(),
    }))
}

/// Core word-processing content — paragraphs, a heading, a bold run, and a
/// table — must survive an ODT export→re-import with no model divergence.
#[test]
fn odt_round_trip_preserves_core_content() {
    let seed = doc(vec![
        Block::Para(vec![Inline::Str("Hello world".to_string())]),
        Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("A heading".to_string())],
        ),
        Block::Para(vec![
            Inline::Str("Some ".to_string()),
            styled_run(
                "bold",
                CharProps {
                    bold: Some(true),
                    ..Default::default()
                },
            ),
            Inline::Str(" text.".to_string()),
        ]),
        table(vec![vec!["A1", "A2"], vec!["B1", "B2"]]),
    ]);

    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "core ODT round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}

/// Secondary run formatting (highlight, letter-spacing, all-caps) and bookmark
/// anchors — which ODT export documents as lossless — must round-trip too. The
/// same content class regressed silently on DOCX export before the symmetric
/// `emit_char_props` fix; this guards the ODF path against the analogue.
#[test]
fn odt_round_trip_preserves_secondary_formatting() {
    use loki_doc_model::style::props::char_props::HighlightColor;
    use loki_primitives::units::Points;

    let seed = doc(vec![
        Block::Para(vec![
            styled_run(
                "highlighted",
                CharProps {
                    highlight_color: Some(HighlightColor::Yellow),
                    ..Default::default()
                },
            ),
            Inline::Str(" and ".to_string()),
            styled_run(
                "spaced",
                CharProps {
                    letter_spacing: Some(Points::new(2.0)),
                    ..Default::default()
                },
            ),
        ]),
        Block::Para(vec![
            Inline::Bookmark(BookmarkKind::Start, "mark1".to_string()),
            styled_run(
                "ALL CAPS",
                CharProps {
                    all_caps: Some(true),
                    ..Default::default()
                },
            ),
            Inline::Bookmark(BookmarkKind::End, "mark1".to_string()),
        ]),
    ]);

    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "secondary ODT round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}
