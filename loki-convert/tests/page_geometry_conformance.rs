// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Page-geometry conformance across the format boundary** (Spec 08 T6.8).
//!
//! T6.8 names five cases — mirrored margins, a custom page size, an N-column
//! section with a separator, multiple page styles in one document, and a
//! DOCX → ODF → DOCX round trip. The fifth is not a sixth case: it is the
//! *path* the other four are asserted along, and it is the only one that
//! exercises both format families against each other.
//!
//! # Why here rather than in `loki-ooxml` or `loki-odf`
//!
//! Neither format crate can see the other — `loki-odf`'s tests do not link
//! `loki-ooxml` — so a same-format round trip is all either can assert, and a
//! same-format round trip cannot catch a property both halves of one crate
//! agree to drop. `loki-convert` is the crate whose whole job is the crossing,
//! and its `matrix_round_trips.rs` already walks this path for *text*; this is
//! that path for geometry.
//!
//! # What this suite found
//!
//! The advisory page-style part (T6.5) built its declared-style list from the
//! document's **catalog** and its section mapping from the **section
//! references**. When those disagreed the exporter emitted a map naming ids it
//! had not declared, which the reader correctly discarded — so names were lost
//! and the loss looked exactly like the feature not existing. Every T6.5 test
//! used a document whose catalog and sections already agreed, which is why a
//! same-format suite could not see it. Fixed in
//! `page_style_part::PageStyleMap::from_document`, with
//! `every_map_the_writer_produces_is_one_the_reader_accepts` as the invariant.

use std::io::Cursor;

use loki_convert::{ConvertOptions, Format, convert};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize, SectionColumns};
use loki_doc_model::layout::section::Section;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::settings::DocumentSettings;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::page_style::PageStyle;
use loki_ooxml::{DocxExport, DocxImport};

/// A page size the paper catalogue cannot name — the "custom size" case, and
/// deliberately not A4 or Letter so a default leaking through is visible.
const CUSTOM_W: f64 = 500.0;
const CUSTOM_H: f64 = 700.0;

/// The T6.8 fixture: two named page styles, one custom-sized three-column
/// section with a separator and asymmetric margins, one A4 section, and
/// mirrored margins on the document.
fn seed() -> Document {
    let mut doc = Document::new();

    let mut body = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize {
                width: Points::new(CUSTOM_W),
                height: Points::new(CUSTOM_H),
            },
            // Asymmetric left/right, so a round trip that swapped or averaged
            // them is visible; symmetric margins would hide both.
            margins: PageMargins {
                top: Points::new(36.0),
                bottom: Points::new(36.0),
                left: Points::new(90.0),
                right: Points::new(54.0),
                ..PageMargins::default()
            },
            columns: Some(SectionColumns {
                count: 3,
                gap: Points::new(18.0),
                separator: true,
                widths: Vec::new(),
            }),
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str("Body section".into())])],
    );
    body.page_style = Some(StyleId::new("Body"));

    let mut cover = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::a4(),
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str("Cover section".into())])],
    );
    cover.page_style = Some(StyleId::new("Cover"));

    doc.sections = vec![body, cover];
    doc.styles.page_styles.insert(
        StyleId::new("Body"),
        PageStyle::new(StyleId::new("Body"), doc.sections[0].layout.clone()),
    );
    let mut cover_style = PageStyle::new(StyleId::new("Cover"), doc.sections[1].layout.clone());
    cover_style.display_name = Some("Cover Page".to_string());
    doc.styles
        .page_styles
        .insert(StyleId::new("Cover"), cover_style);

    doc.settings = Some(DocumentSettings {
        mirror_margins: true,
        ..DocumentSettings::default()
    });
    doc
}

fn to_docx(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("DOCX export");
    buf.into_inner()
}

/// DOCX → ODT → DOCX through the real conversion matrix, returning the
/// re-imported document.
fn docx_odt_docx(doc: &Document) -> Document {
    let docx = to_docx(doc);
    let odt =
        convert(Format::Docx, &docx, Format::Odt, &ConvertOptions::default()).expect("DOCX → ODT");
    assert_eq!(&odt.bytes[..2], b"PK", "ODT output is not a ZIP");
    let back = convert(
        Format::Odt,
        &odt.bytes,
        Format::Docx,
        &ConvertOptions::default(),
    )
    .expect("ODT → DOCX");
    DocxImport::import(Cursor::new(&back.bytes), Default::default()).expect("re-import")
}

/// Sections survive the crossing at all — every other assertion indexes into
/// them, so a lost section would otherwise show up as a confusing panic.
#[test]
fn the_section_structure_survives_the_crossing() {
    let back = docx_odt_docx(&seed());
    assert_eq!(back.sections.len(), 2, "a section was lost or invented");
}

/// **Custom page size** — a size the paper catalogue cannot name must cross
/// unchanged, in points, not snapped to the nearest standard paper.
#[test]
fn a_custom_page_size_crosses_unchanged() {
    let back = docx_odt_docx(&seed());
    let size = &back.sections[0].layout.page_size;
    assert!(
        (size.width.value() - CUSTOM_W).abs() < 1.0 && (size.height.value() - CUSTOM_H).abs() < 1.0,
        "custom size became {:.1}×{:.1}",
        size.width.value(),
        size.height.value()
    );
    // The catalogue must still decline to name it — a size that acquired a name
    // in transit is a size that was quietly rounded to a standard paper.
    assert!(
        size.paper().is_none(),
        "the custom size came back as a catalogued paper: {:?}",
        size.paper().map(|p| p.id)
    );
    // The second section's A4 is unchanged too, so the first is not simply
    // whatever both became.
    assert_eq!(
        back.sections[1].layout.page_size.paper().map(|p| p.id),
        Some("a4")
    );
}

/// **Asymmetric margins** cross without being swapped or equalised.
#[test]
fn asymmetric_margins_cross_unchanged() {
    let back = docx_odt_docx(&seed());
    let m = &back.sections[0].layout.margins;
    for (got, want, edge) in [
        (m.top.value(), 36.0, "top"),
        (m.bottom.value(), 36.0, "bottom"),
        (m.left.value(), 90.0, "left"),
        (m.right.value(), 54.0, "right"),
    ] {
        assert!(
            (got - want).abs() < 1.0,
            "{edge} margin became {got:.1}, expected {want:.1}"
        );
    }
}

/// **N-column with separator** — both the count and the separator flag. The
/// separator is the half that a column implementation carrying only `count`
/// would silently drop.
#[test]
fn a_three_column_section_with_a_separator_crosses_unchanged() {
    let back = docx_odt_docx(&seed());
    let cols = back.sections[0]
        .layout
        .columns
        .as_ref()
        .expect("the three-column section lost its columns entirely");
    assert_eq!(cols.count, 3, "column count changed");
    assert!(cols.separator, "the column separator was dropped");
    // The single-column section must not have acquired columns.
    assert!(
        back.sections[1]
            .layout
            .columns
            .as_ref()
            .is_none_or(|c| c.count <= 1),
        "the single-column section came back multi-column"
    );
}

/// **Mirrored margins** (T6.1's ODF `style:page-usage` work) survive the
/// crossing in the direction that has to union two origins: DOCX states it once
/// per document in `settings.xml`, ODF states it per page layout.
#[test]
fn mirrored_margins_cross_unchanged() {
    let back = docx_odt_docx(&seed());
    assert!(
        back.mirrors_margins(),
        "mirrored margins were lost crossing DOCX → ODT → DOCX"
    );

    // The polarity: a document that did *not* ask for mirroring must not come
    // back mirrored. Without this the assertion above passes for an
    // implementation that always reports true.
    let mut plain = seed();
    plain.settings = None;
    for s in &mut plain.sections {
        s.layout.page_usage = loki_doc_model::layout::page_usage::PageUsage::All;
    }
    assert!(
        !docx_odt_docx(&plain).mirrors_margins(),
        "an unmirrored document came back mirrored"
    );
}

/// **Multiple page styles per document** — the case the advisory part (T6.5)
/// exists for, asserted across the boundary rather than within one format.
///
/// This is the assertion that failed before `from_document` was fixed: the
/// names came back as ODF's positional defaults (`Standard`, `MP1`).
#[test]
fn multiple_named_page_styles_cross_unchanged() {
    let back = docx_odt_docx(&seed());
    assert_eq!(
        back.sections[0].page_style.as_ref().map(StyleId::as_str),
        Some("Body"),
        "the first page-style name was lost crossing formats"
    );
    assert_eq!(
        back.sections[1].page_style.as_ref().map(StyleId::as_str),
        Some("Cover")
    );
    // Two *distinct* styles, not one applied twice — a round trip that collapsed
    // them would still satisfy a test that only checked the first.
    assert_ne!(back.sections[0].page_style, back.sections[1].page_style);
    assert!(
        back.styles.page_styles.contains_key(&StyleId::new("Body"))
            && back.styles.page_styles.contains_key(&StyleId::new("Cover")),
        "the catalog did not gain both styles: {:?}",
        back.styles.page_styles.keys().collect::<Vec<_>>()
    );
}

/// **The catalog and the section references may disagree**, and the crossing
/// must still carry the names.
///
/// This is the fixture the rest of this suite does *not* use: `seed()`
/// populates the catalog, so every other test here passes with or without the
/// `from_document` fix. An importer that sets `section.page_style` without
/// registering a catalog entry produces exactly this shape, and it is the shape
/// that lost its names — so without this case the suite would document the fix
/// while being unable to detect its absence.
#[test]
fn names_cross_even_when_the_catalog_is_empty() {
    let mut doc = seed();
    doc.styles.page_styles.clear();
    let back = docx_odt_docx(&doc);
    assert_eq!(
        back.sections[0].page_style.as_ref().map(StyleId::as_str),
        Some("Body"),
        "names were lost when the catalog did not list them"
    );
    assert_eq!(
        back.sections[1].page_style.as_ref().map(StyleId::as_str),
        Some("Cover")
    );
}

/// Crossing twice must be a fixed point: the second crossing changes nothing
/// the first did not. A property that degrades a little each pass — a margin
/// rounded, a name suffixed — looks stable in a single-crossing test.
#[test]
fn a_second_crossing_changes_nothing() {
    let once = docx_odt_docx(&seed());
    let twice = docx_odt_docx(&once);

    assert_eq!(once.sections.len(), twice.sections.len());
    for (i, (a, b)) in once.sections.iter().zip(twice.sections.iter()).enumerate() {
        assert_eq!(
            a.layout, b.layout,
            "section {i} geometry drifted on the second crossing"
        );
        assert_eq!(
            a.page_style, b.page_style,
            "section {i} page-style name drifted on the second crossing"
        );
    }
    assert_eq!(once.mirrors_margins(), twice.mirrors_margins());
}
