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
///
/// **Stability, not fidelity**, and the distinction is not academic: this
/// compares first-import against re-import, so a property dropped on the *first*
/// export is absent from both and the comparison agrees. It passed for years
/// while `highlight_color` reached no ODT file at all (Spec 08 T5.3). The
/// fidelity half is `a_named_highlight_survives_odt_export` below.
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

/// **A named highlight reaches the exported file.** ODF has one text-background
/// attribute where OOXML has two, so a `w:highlight` colour has to travel as
/// `fo:background-color` — and until Spec 08 T5.3 it travelled as nothing:
/// `highlight_color` appeared nowhere in this crate's writer.
///
/// Asserted on the *imported* model rather than on the XML, so it is a statement
/// about what survives rather than about how it is spelled.
#[test]
fn a_named_highlight_survives_odt_export() {
    use loki_doc_model::style::props::char_props::HighlightColor;

    let seed = doc(vec![Block::Para(vec![styled_run(
        "highlighted",
        CharProps {
            highlight_color: Some(HighlightColor::Yellow),
            ..Default::default()
        },
    )])]);

    let back = import(export(&seed));
    let background = effective_background(&back);

    // Yellow, as a colour — **not** as `HighlightColor::Yellow`. ODF cannot
    // distinguish a highlight from a character background, so the named variant
    // becomes shading on the way back in. That is the format, not a loss: the
    // colour the reader sees is preserved exactly, and a DOCX re-export puts it
    // in Word's Shading control rather than its Highlight control.
    let hex = background
        .as_ref()
        .and_then(loki_primitives::color::DocumentColor::to_hex);
    assert_eq!(
        hex.as_deref(),
        Some("#FFFF00"),
        "the highlight colour did not survive ODT export: {background:?}"
    );
}

/// **And a run with no highlight gains no background** — otherwise the test
/// above would pass for a writer that paints every run yellow.
#[test]
fn an_unhighlighted_run_gains_no_background() {
    let seed = doc(vec![Block::Para(vec![styled_run(
        "plain",
        CharProps {
            all_caps: Some(true),
            ..Default::default()
        },
    )])]);

    let back = import(export(&seed));
    assert_eq!(effective_background(&back), None);
}

/// The first styled run's **effective** character background.
///
/// Resolved through the style catalog, not read off `direct_props`: the ODT
/// writer emits run formatting as an automatic `style:family="text"` style and
/// the reader returns it as a `StyleId` reference with no direct props, so a
/// direct-props-only reader reports `None` for a colour that is plainly there.
/// The first draft of this helper did exactly that and accused the reader of a
/// loss the writer had just stopped causing.
fn effective_background(d: &Document) -> Option<loki_primitives::color::DocumentColor> {
    fn first_run(inlines: &[Inline]) -> Option<&StyledRun> {
        inlines.iter().find_map(|i| match i {
            Inline::StyledRun(run) => Some(run),
            _ => None,
        })
    }
    let run = d.sections.iter().find_map(|s| {
        s.blocks.iter().find_map(|b| match b {
            Block::Para(inlines) => first_run(inlines),
            Block::StyledPara(sp) => first_run(&sp.inlines),
            _ => None,
        })
    })?;
    if let Some(direct) = run
        .direct_props
        .as_ref()
        .and_then(|p| p.background_color.clone())
    {
        return Some(direct);
    }
    let id = run.style_id.as_ref()?;
    d.styles
        .character_styles
        .get(id)
        .and_then(|s| s.char_props.background_color.clone())
}
