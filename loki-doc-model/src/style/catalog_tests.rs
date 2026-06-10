// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`crate::style::catalog`].

use super::*;
use crate::content::attr::ExtensionBag;
use loki_primitives::units::Points;

fn make_catalog_with_parent_child() -> StyleCatalog {
    let mut catalog = StyleCatalog::new();

    let parent = ParagraphStyle {
        id: StyleId::new("Normal"),
        display_name: Some("Normal".into()),
        parent: None,
        linked_char_style: None,
        next_style_id: None,
        para_props: ParaProps::default(),
        char_props: CharProps {
            font_size: Some(Points::new(12.0)),
            bold: Some(false),
            ..Default::default()
        },
        is_default: true,
        is_custom: false,
        extensions: ExtensionBag::default(),
    };

    let child = ParagraphStyle {
        id: StyleId::new("Heading1"),
        display_name: Some("Heading 1".into()),
        parent: Some(StyleId::new("Normal")),
        linked_char_style: None,
        next_style_id: None,
        para_props: ParaProps::default(),
        char_props: CharProps {
            font_size: Some(Points::new(24.0)),
            bold: Some(true),
            ..Default::default()
        },
        is_default: false,
        is_custom: false,
        extensions: ExtensionBag::default(),
    };

    catalog
        .paragraph_styles
        .insert(StyleId::new("Normal"), parent);
    catalog
        .paragraph_styles
        .insert(StyleId::new("Heading1"), child);
    catalog
}

#[test]
fn resolve_child_overrides_parent() {
    let catalog = make_catalog_with_parent_child();
    let resolved = catalog.resolve_char(&StyleId::new("Heading1")).unwrap();
    assert_eq!(resolved.font_size, Some(Points::new(24.0)));
    assert_eq!(resolved.bold, Some(true));
}

#[test]
fn resolve_child_inherits_parent_unset() {
    let catalog = make_catalog_with_parent_child();
    // The parent has font_size=12pt. The child overrides to 24pt.
    // But italic is None in both — should still be None after resolution.
    let resolved = catalog.resolve_char(&StyleId::new("Heading1")).unwrap();
    assert!(resolved.italic.is_none());
}

#[test]
fn resolve_missing_style_returns_none() {
    let catalog = StyleCatalog::new();
    assert!(catalog.resolve_para(&StyleId::new("NonExistent")).is_none());
}

/// Build a catalog with a parent cycle: A.parent = B, B.parent = A.
fn make_cyclic_catalog() -> StyleCatalog {
    let mut catalog = StyleCatalog::new();
    let mk = |id: &str, parent: &str, size: f64| ParagraphStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: Some(StyleId::new(parent)),
        linked_char_style: None,
        next_style_id: None,
        para_props: ParaProps::default(),
        char_props: CharProps {
            font_size: Some(Points::new(size)),
            ..Default::default()
        },
        is_default: false,
        is_custom: false,
        extensions: ExtensionBag::default(),
    };
    catalog
        .paragraph_styles
        .insert(StyleId::new("A"), mk("A", "B", 10.0));
    catalog
        .paragraph_styles
        .insert(StyleId::new("B"), mk("B", "A", 20.0));
    catalog
}

#[test]
fn resolve_char_cyclic_parents_terminates() {
    let catalog = make_cyclic_catalog();
    // Must terminate (no stack overflow) and keep the child's own value.
    let resolved = catalog.resolve_char(&StyleId::new("A")).unwrap();
    assert_eq!(resolved.font_size, Some(Points::new(10.0)));
    // Fields unset anywhere in the cycle stay unset.
    assert!(resolved.bold.is_none());
}

#[test]
fn resolve_para_cyclic_parents_terminates() {
    let catalog = make_cyclic_catalog();
    let resolved = catalog.resolve_para(&StyleId::new("B")).unwrap();
    // Sane output: defaults, since neither style sets para props.
    assert!(resolved.alignment.is_none());
}

#[test]
fn resolve_self_referential_parent_terminates() {
    let mut catalog = StyleCatalog::new();
    catalog.paragraph_styles.insert(
        StyleId::new("Loop"),
        ParagraphStyle {
            id: StyleId::new("Loop"),
            display_name: None,
            parent: Some(StyleId::new("Loop")),
            linked_char_style: None,
            next_style_id: None,
            para_props: ParaProps::default(),
            char_props: CharProps {
                bold: Some(true),
                ..Default::default()
            },
            is_default: false,
            is_custom: false,
            extensions: ExtensionBag::default(),
        },
    );
    let resolved = catalog.resolve_char(&StyleId::new("Loop")).unwrap();
    assert_eq!(resolved.bold, Some(true));
}

#[test]
fn resolve_deep_legitimate_chain_still_inherits() {
    // A linear (acyclic) chain shorter than the cap must fully inherit.
    let mut catalog = StyleCatalog::new();
    for i in 0..10 {
        let parent = if i == 0 {
            None
        } else {
            Some(StyleId::new(format!("S{}", i - 1)))
        };
        let char_props = if i == 0 {
            CharProps {
                italic: Some(true),
                ..Default::default()
            }
        } else {
            CharProps::default()
        };
        catalog.paragraph_styles.insert(
            StyleId::new(format!("S{i}")),
            ParagraphStyle {
                id: StyleId::new(format!("S{i}")),
                display_name: None,
                parent,
                linked_char_style: None,
                next_style_id: None,
                para_props: ParaProps::default(),
                char_props,
                is_default: false,
                is_custom: false,
                extensions: ExtensionBag::default(),
            },
        );
    }
    let resolved = catalog.resolve_char(&StyleId::new("S9")).unwrap();
    assert_eq!(resolved.italic, Some(true), "root value must inherit down");
}
