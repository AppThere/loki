// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the named page style (ADR-0012 Decision 2, page family).

use super::*;
use crate::layout::page::{PageOrientation, PageSize};
use crate::layout::section::Section;
use crate::style::catalog::{StyleCatalog, StyleId};

fn section_with(size: PageSize) -> Section {
    Section::with_layout_and_blocks(
        PageLayout {
            page_size: size,
            ..Default::default()
        },
        Vec::new(),
    )
}

#[test]
fn new_wraps_a_layout_with_no_display_name() {
    let layout = PageLayout {
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    let ps = PageStyle::new(StyleId::new("Landscape"), layout.clone());
    assert_eq!(ps.id, StyleId::new("Landscape"));
    assert_eq!(ps.display_name, None);
    assert_eq!(ps.layout.orientation, PageOrientation::Landscape);
}

#[test]
fn page_styles_live_in_the_catalog_keyed_by_id() {
    let mut cat = StyleCatalog::new();
    let a4 = PageLayout {
        page_size: PageSize::a4(),
        ..Default::default()
    };
    cat.page_styles
        .insert(StyleId::new("A4"), PageStyle::new(StyleId::new("A4"), a4));
    assert_eq!(cat.page_styles.len(), 1);
    let got = cat.page_styles.get(&StyleId::new("A4")).unwrap();
    assert_eq!(got.layout.page_size, PageSize::a4());
    // Page styles are non-inheriting: the struct has no `parent` field at all.
}

#[test]
fn default_page_styles_catalog_is_empty() {
    let cat = StyleCatalog::new();
    assert!(cat.page_styles.is_empty());
}

#[test]
fn derive_dedups_identical_layouts_and_names_in_order() {
    // A4, Letter, A4 again → two distinct page styles (the third reuses the first).
    let sections = vec![
        section_with(PageSize::a4()),
        section_with(PageSize::letter()),
        section_with(PageSize::a4()),
    ];
    let styles = derive_page_styles(&sections);
    assert_eq!(styles.len(), 2);
    let names: Vec<&str> = styles.keys().map(StyleId::as_str).collect();
    assert_eq!(names, vec!["PageStyle1", "PageStyle2"]);
    assert_eq!(
        styles
            .get(&StyleId::new("PageStyle1"))
            .unwrap()
            .layout
            .page_size,
        PageSize::a4()
    );
}

#[test]
fn section_ids_map_each_section_to_its_page_style() {
    let sections = vec![
        section_with(PageSize::a4()),
        section_with(PageSize::letter()),
        section_with(PageSize::a4()),
    ];
    let ids: Vec<String> = section_page_style_ids(&sections)
        .iter()
        .map(|id| id.as_str().to_string())
        .collect();
    // The third section shares the first's page style (identical A4 layout).
    assert_eq!(ids, vec!["PageStyle1", "PageStyle2", "PageStyle1"]);
}

#[test]
fn derive_on_no_sections_is_empty() {
    assert!(derive_page_styles(&[]).is_empty());
    assert!(section_page_style_ids(&[]).is_empty());
}
