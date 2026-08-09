// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the advisory page-style map (Spec 08 T6.5, D-02) — what it records
//! and what it refuses to apply.

use super::{PageStyleMap, PartStyle, apply_page_style_part};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleId;

/// Two sections with distinct geometry, named `Body` and `Cover`.
fn two_section_doc() -> Document {
    let mut doc = Document::new();
    let section = |size: PageSize| {
        Section::with_layout_and_blocks(
            PageLayout {
                page_size: size,
                ..Default::default()
            },
            vec![Block::Para(vec![Inline::Str("x".into())])],
        )
    };
    doc.sections = vec![section(PageSize::a4()), section(PageSize::letter())];
    doc.assign_page_styles();
    doc.sections[0].page_style = Some(StyleId::new("Body"));
    doc.sections[1].page_style = Some(StyleId::new("Cover"));
    doc
}

fn map_of(pairs: &[(&str, Option<&str>)], sections: &[Option<&str>]) -> PageStyleMap {
    PageStyleMap {
        styles: pairs
            .iter()
            .map(|(id, dn)| PartStyle {
                id: (*id).to_string(),
                display_name: dn.map(str::to_string),
            })
            .collect(),
        sections: sections.iter().map(|s| s.map(str::to_string)).collect(),
    }
}

#[test]
fn a_document_with_named_page_styles_produces_a_map() {
    let doc = two_section_doc();
    let map = PageStyleMap::from_document(&doc).expect("has names");
    assert_eq!(
        map.sections,
        vec![Some("Body".to_string()), Some("Cover".to_string())]
    );
    let xml = map.to_xml();
    assert!(xml.contains(r#"sectionCount="2""#), "{xml}");
    assert!(
        xml.contains(r#"<section index="0" style="Body"/>"#),
        "{xml}"
    );
}

/// A document nobody named gets no part — an empty one is something every
/// future reader has to consider and reject.
#[test]
fn a_document_with_no_named_page_styles_produces_nothing() {
    let mut doc = two_section_doc();
    doc.sections[0].page_style = None;
    doc.sections[1].page_style = None;
    assert!(PageStyleMap::from_document(&doc).is_none());
}

/// **The load-bearing invariant: advisory means names only.** Every layout field
/// of every section must be identical before and after — a part that could
/// change geometry would be a second, invisible source for the page.
#[test]
fn the_part_never_changes_geometry() {
    let mut doc = two_section_doc();
    let before: Vec<PageLayout> = doc.sections.iter().map(|s| s.layout.clone()).collect();

    // A map whose styles are named nothing like the current ones, and whose
    // order is reversed — the most disruptive thing a valid map can say.
    let map = map_of(
        &[("Alpha", Some("Front Matter")), ("Beta", None)],
        &[Some("Beta"), Some("Alpha")],
    );
    assert!(apply_page_style_part(&mut doc, &map), "should have applied");

    let after: Vec<PageLayout> = doc.sections.iter().map(|s| s.layout.clone()).collect();
    assert_eq!(before, after, "the advisory part changed page geometry");
    // ...and the names *did* move, so the comparison above is not vacuous.
    assert_eq!(doc.sections[0].page_style, Some(StyleId::new("Beta")));
    assert_eq!(doc.sections[1].page_style, Some(StyleId::new("Alpha")));
    assert_eq!(
        doc.styles
            .page_styles
            .get(&StyleId::new("Alpha"))
            .and_then(|p| p.display_name.clone()),
        Some("Front Matter".to_string())
    );
}

/// **Validated against the section count, discarded on mismatch.** Word can
/// split or merge sections; a map describing a different number of them is
/// describing a different document.
#[test]
fn a_section_count_mismatch_discards_the_whole_part() {
    for sections in [
        vec![Some("Body")],                              // too few
        vec![Some("Body"), Some("Cover"), Some("Body")], // too many
    ] {
        let mut doc = two_section_doc();
        let before: Vec<_> = doc.sections.iter().map(|s| s.page_style.clone()).collect();
        let map = map_of(&[("Body", None), ("Cover", None)], &sections);
        assert!(
            !apply_page_style_part(&mut doc, &map),
            "a {}-section map was applied to a 2-section document",
            sections.len()
        );
        let after: Vec<_> = doc.sections.iter().map(|s| s.page_style.clone()).collect();
        assert_eq!(before, after, "a rejected map still changed the document");
    }
}

/// A section naming a style the part never declared is an inconsistent map.
/// The **whole** part goes, not the one entry: a partial mapping leaves some
/// sections named and others not, which is harder to explain than no names.
#[test]
fn an_undeclared_style_reference_discards_the_whole_part() {
    let mut doc = two_section_doc();
    let before: Vec<_> = doc.sections.iter().map(|s| s.page_style.clone()).collect();
    // `Cover` is referenced but only `Body` is declared.
    let map = map_of(&[("Body", None)], &[Some("Body"), Some("Cover")]);
    assert!(!apply_page_style_part(&mut doc, &map));
    let after: Vec<_> = doc.sections.iter().map(|s| s.page_style.clone()).collect();
    assert_eq!(
        before, after,
        "the valid half of an inconsistent map was applied"
    );
}

/// A map may legitimately leave a section unnamed; that is not a mismatch.
#[test]
fn a_section_with_no_style_is_allowed() {
    let mut doc = two_section_doc();
    let map = map_of(&[("Body", None)], &[Some("Body"), None]);
    assert!(apply_page_style_part(&mut doc, &map));
    assert_eq!(doc.sections[0].page_style, Some(StyleId::new("Body")));
    assert_eq!(doc.sections[1].page_style, None);
}

/// The XML escapes names — a page style called `A & B "quoted"` must not
/// produce a part that fails to parse.
#[test]
fn names_are_escaped_in_the_serialised_part() {
    let map = map_of(
        &[(r#"A & B"#, Some(r#"the "real" name"#))],
        &[Some(r#"A & B"#)],
    );
    let xml = map.to_xml();
    assert!(xml.contains("A &amp; B"), "{xml}");
    assert!(xml.contains("&quot;real&quot;"), "{xml}");
    assert!(
        !xml.contains(r#"id="A & B""#),
        "raw ampersand survived: {xml}"
    );
}

/// **Whatever the writer produces, the reader must accept — on the same
/// document.** This is the property, not a case: the two halves were written
/// against different sources (`styles` from the catalog, `sections` from the
/// section references) and disagreed whenever those two disagreed, so the
/// exporter emitted a map guaranteed to be discarded on reimport. Found by a
/// DOCX → ODT → DOCX probe during T6.8, not by any of T6.5's own tests, every
/// one of which used a document whose catalog and sections already agreed.
#[test]
fn every_map_the_writer_produces_is_one_the_reader_accepts() {
    // A section naming a style the catalog has no entry for — the exact
    // disagreement the two sources allowed.
    let mut uncatalogued = two_section_doc();
    uncatalogued.styles.page_styles.clear();

    // ...and one where the catalog holds *extra* styles no section uses, which
    // must not make the map inconsistent either.
    let mut extra = two_section_doc();
    extra.styles.page_styles.insert(
        StyleId::new("Unused"),
        loki_doc_model::style::page_style::PageStyle::new(
            StyleId::new("Unused"),
            PageLayout::default(),
        ),
    );

    for (mut doc, why) in [
        (two_section_doc(), "catalog and sections agree"),
        (uncatalogued, "sections name uncatalogued styles"),
        (extra, "catalog holds unused styles"),
    ] {
        let map = PageStyleMap::from_document(&doc).expect("has names");
        assert!(
            apply_page_style_part(&mut doc, &map),
            "the writer produced a map its own reader rejects ({why})"
        );
        // And the names really did survive the round trip through the map.
        assert_eq!(doc.sections[0].page_style, Some(StyleId::new("Body")));
        assert_eq!(doc.sections[1].page_style, Some(StyleId::new("Cover")));
    }
}
