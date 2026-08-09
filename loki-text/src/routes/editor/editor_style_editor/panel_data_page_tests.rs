// SPDX-License-Identifier: Apache-2.0

//! Tests for the page family's panel data — which styles the browser lists,
//! which geometry the form edits, and which section "apply here" targets.

use std::sync::{Arc, Mutex};

use super::{caret_section_index, next_page_style_name, panel_page_styles};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::StyleId;
use loki_doc_model::style::page_style::PageStyle;

/// A `DocumentState` holding `doc`, shared the way the panel receives it.
fn shared(doc: Document) -> Arc<Mutex<crate::editing::state::DocumentState>> {
    let mut state = crate::editing::state::DocumentState::new();
    state.document = Some(std::sync::Arc::new(doc));
    Arc::new(Mutex::new(state))
}

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.into())])
}

/// Two A4 sections sharing `PageStyle1`, with the catalog entry assigned.
fn doc_two_sections() -> Document {
    let mut doc = Document::new();
    doc.sections = vec![
        Section::with_layout_and_blocks(PageLayout::default(), vec![para("a"), para("b")]),
        Section::with_layout_and_blocks(PageLayout::default(), vec![para("c")]),
    ];
    doc.assign_page_styles();
    doc
}

/// The section wins over the catalog entry for an applied style.
///
/// The sections are moved to **A4** rather than Letter: `PageSize::default()`
/// *is* Letter, so the first version of this test set the sections to the value
/// the catalog already held and passed whichever source the code read — a
/// control that silenced its own subject. The guard below asserts the two
/// genuinely differ before the real assertion reads one of them.
#[test]
fn an_applied_style_reads_its_geometry_from_the_section() {
    let mut doc = doc_two_sections();
    // The Layout ribbon path: change the sections, leave the catalog entry alone.
    doc.sections[0].layout.page_size = PageSize::a4();
    doc.sections[1].layout.page_size = PageSize::a4();

    let catalogued = doc
        .styles
        .page_styles
        .get(&StyleId::new("PageStyle1"))
        .map(|ps| ps.layout.page_size.width.value())
        .expect("catalogued");
    assert!(
        (catalogued - PageSize::a4().width.value()).abs() > 1.0,
        "fixture broken: the catalog entry already holds the section's geometry, \
         so this test cannot tell the two sources apart"
    );

    let styles = panel_page_styles(&doc);
    assert_eq!(styles.len(), 1);
    assert_eq!(styles[0].sections, vec![0, 1]);
    assert!(
        (styles[0].layout.page_size.width.value() - PageSize::a4().width.value()).abs() < 1.0,
        "panel showed the catalog's stale geometry instead of the section's"
    );
}

/// A style no section references must still be listed — it is the only handle
/// on a style that was just created, and an unlisted style cannot be applied.
#[test]
fn an_unapplied_style_is_listed_with_its_catalog_geometry() {
    let mut doc = doc_two_sections();
    let landscape = PageLayout {
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    doc.styles.page_styles.insert(
        StyleId::new("Landscape"),
        PageStyle::new(StyleId::new("Landscape"), landscape),
    );

    let styles = panel_page_styles(&doc);
    let found = styles
        .iter()
        .find(|p| p.id.as_str() == "Landscape")
        .expect("unapplied style must be listed");
    assert!(found.sections.is_empty());
    assert_eq!(found.layout.orientation, PageOrientation::Landscape);
}

/// A section may name a style the catalog lacks (an importer that wrote the
/// reference but not the entry). Dropping it would leave those pages with no
/// reachable page style at all.
#[test]
fn a_referenced_but_uncatalogued_style_is_still_listed() {
    let mut doc = doc_two_sections();
    doc.sections[1].page_style = Some(StyleId::new("Orphan"));

    let styles = panel_page_styles(&doc);
    let found = styles
        .iter()
        .find(|p| p.id.as_str() == "Orphan")
        .expect("uncatalogued reference must still be listed");
    assert_eq!(found.sections, vec![1]);
}

#[test]
fn the_next_name_skips_every_taken_one() {
    let mut doc = doc_two_sections();
    for n in [2, 3] {
        let id = StyleId::new(format!("PageStyle{n}"));
        doc.styles
            .page_styles
            .insert(id.clone(), PageStyle::new(id, PageLayout::default()));
    }
    // PageStyle1..3 are taken; the count (3) would collide, so it must step past.
    let state = shared(doc);
    assert_eq!(next_page_style_name(&state).as_deref(), Some("PageStyle4"));
}

/// The caret's `paragraph_index` is a *flat* block index across sections, so a
/// caret in the second section's only block (flat 2) must resolve to section 1,
/// not section 2 — the failure an index-as-section-number reading would give.
#[test]
fn the_caret_section_comes_from_the_flat_block_index() {
    let doc = doc_two_sections();
    assert_eq!(doc.flat_index_to_section_block(0), Some((0, 0)));
    assert_eq!(doc.flat_index_to_section_block(1), Some((0, 1)));
    assert_eq!(doc.flat_index_to_section_block(2), Some((1, 0)));
    assert_eq!(doc.flat_index_to_section_block(3), None);

    let state = shared(doc);
    let pos = crate::editing::cursor::DocumentPosition::top_level(0, 2, 0);
    assert_eq!(caret_section_index(&state, Some(&pos)), Some(1));
    // No caret yet → no target, rather than a silent section 0.
    assert_eq!(caret_section_index(&state, None), None);
}
