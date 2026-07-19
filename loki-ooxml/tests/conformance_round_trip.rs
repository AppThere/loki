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

/// Emboss, imprint, and character-border (`w:bdr`) direct run formatting must
/// survive import-export-import. Each used to export with the field silently
/// dropped (import + render only), so a run carrying *only* one of these would
/// collapse to a plain run and merge with its neighbours — the same class of
/// loss the secondary-formatting regression above guards. Now `emit_char_props`
/// writes `<w:emboss>`/`<w:imprint>`/`<w:bdr>` symmetrically with the reader.
#[test]
fn docx_round_trip_preserves_emboss_imprint_and_char_border() {
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_primitives::color::DocumentColor;

    let embossed = CharProps {
        emboss: Some(true),
        ..Default::default()
    };
    let imprinted = CharProps {
        imprint: Some(true),
        ..Default::default()
    };
    let bordered = CharProps {
        character_border: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.0),
            color: Some(DocumentColor::from_hex("#C00000").expect("valid hex")),
            spacing: Some(Points::new(1.0)),
        }),
        ..Default::default()
    };

    let seed = doc(vec![Block::Para(vec![
        styled_run("embossed", embossed),
        Inline::Str(" ".to_string()),
        styled_run("imprinted", imprinted),
        Inline::Str(" ".to_string()),
        styled_run("boxed", bordered),
        Inline::Str(" plain tail.".to_string()),
    ])]);

    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "emboss/imprint/char-border diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}

/// Flattens a block's inline content, seeing through the `StyledPara` wrapper
/// the importer produces for a bare paragraph as well as plain `Para`/`Plain`.
fn block_inlines(b: &Block) -> Vec<Inline> {
    match b {
        Block::Para(inl) | Block::Plain(inl) => inl.clone(),
        Block::StyledPara(sp) => sp.inlines.clone(),
        _ => vec![],
    }
}

/// A floating `wps` text box (`Inline::TextBox`) must survive
/// import-export-import: the export writes a `w:drawing`/`wp:anchor` whose
/// `wps:wsp` graphicData carries the shape fill/border and a `w:txbxContent`
/// body, and re-import reconstructs the same `TextBox` (geometry, fill, border,
/// interior paragraph text). Used to be import + render only — the box was
/// silently dropped on export.
#[test]
fn docx_round_trip_preserves_floating_text_box() {
    use loki_doc_model::content::inline::Inline;

    // Build a TextBox exactly as the mapper does: geometry + fill/border on the
    // attr, a floating (square) wrap class, one interior paragraph.
    let mut attr = NodeAttr::default();
    attr.kv.push(("cx_emu".to_string(), "1828800".to_string()));
    attr.kv.push(("cy_emu".to_string(), "731520".to_string()));
    attr.kv
        .push(("textbox-fill".to_string(), "FDF0E6".to_string()));
    attr.kv
        .push(("textbox-line".to_string(), "ED7D31".to_string()));
    attr.classes.push("floating".to_string());
    let text_box = Inline::TextBox(
        attr,
        vec![Block::Para(vec![Inline::Str("Sidebar body.".to_string())])],
    );

    let seed = doc(vec![Block::Para(vec![
        text_box,
        Inline::Str("Body copy beside the box.".to_string()),
    ])]);

    let a = import(export(&seed));

    // The re-imported model must still carry a TextBox with the fill/border and
    // the interior text — assert directly (not only via divergence) so a silent
    // downgrade to a plain image or dropped box fails loudly.
    let found = a
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .flat_map(block_inlines)
        .find_map(|i| match i {
            Inline::TextBox(at, blocks) => Some((at, blocks)),
            _ => None,
        });
    let (at, blocks) = found.expect("re-imported model still has a TextBox");
    assert!(
        at.kv
            .iter()
            .any(|(k, v)| k == "textbox-fill" && v == "FDF0E6"),
        "fill survived: {:?}",
        at.kv
    );
    assert!(
        at.kv
            .iter()
            .any(|(k, v)| k == "textbox-line" && v == "ED7D31"),
        "border survived: {:?}",
        at.kv
    );
    assert!(
        at.kv.iter().any(|(k, v)| k == "cx_emu" && v == "1828800"),
        "geometry survived: {:?}",
        at.kv
    );
    let inner: String = blocks
        .iter()
        .flat_map(block_inlines)
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s),
            _ => None,
        })
        .collect();
    assert!(
        inner.contains("Sidebar body."),
        "interior text survived: {inner:?}"
    );

    // And the whole thing must be import-export-import *stable*.
    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "text-box round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}

/// The comprehensive reference fixture (headers, footnotes, hyperlinks, images,
/// …) under the same import-export-import comparison.
///
/// This started as an `#[ignore]`'d gap-finder and is now a **green guard**: the
/// conformance harness surfaced two real export bugs which have since been fixed
/// — the content-loss gap at `blk0005` (highlight / letter-spacing runs
/// collapsing and dropping adjacent text; fixed by making `emit_char_props`
/// symmetric with the reader) and the footnote-reference instability at
/// `blk0026` (export hard-coded an explicit `<w:vertAlign>` the source model
/// never had; the superscript now lives only in the always-emitted
/// `FootnoteReference` character style). The full reference fixture now
/// round-trips with **no model divergence**. Any future export regression that
/// drops or fabricates a canonicalised property fails here with a model path.
#[test]
fn docx_reference_round_trip_is_stable() {
    let a = import(helpers::build_reference_docx());
    let b = import(export(&a));
    if let Some(d) = first_divergence(&canonicalize_document(&a), &canonicalize_document(&b)) {
        panic!(
            "reference DOCX round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}
