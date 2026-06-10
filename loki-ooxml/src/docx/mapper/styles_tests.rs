// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use crate::docx::model::styles::{DocxStyle, DocxStyleType};

fn make_styles(style_type: DocxStyleType, id: &str, name: &str) -> DocxStyles {
    DocxStyles {
        default_rpr: None,
        default_ppr: None,
        styles: vec![DocxStyle {
            style_type,
            style_id: id.into(),
            is_default: false,
            is_custom: false,
            name: Some(name.into()),
            based_on: None,
            next: None,
            link: None,
            ppr: None,
            rpr: None,
        }],
    }
}

#[test]
fn paragraph_style_in_catalog() {
    let styles = make_styles(DocxStyleType::Paragraph, "Normal", "Normal");
    let catalog = map_styles(&styles);
    let s = catalog
        .paragraph_styles
        .get(&StyleId::new("Normal"))
        .unwrap();
    assert_eq!(s.id, StyleId::new("Normal"));
    assert_eq!(s.display_name.as_deref(), Some("Normal"));
    assert!(!s.is_default);
}

#[test]
fn paragraph_style_with_parent() {
    let styles = DocxStyles {
        styles: vec![DocxStyle {
            style_type: DocxStyleType::Paragraph,
            style_id: "Heading1".into(),
            is_default: false,
            is_custom: false,
            name: Some("Heading 1".into()),
            based_on: Some("Normal".into()),
            next: None,
            link: None,
            ppr: None,
            rpr: None,
        }],
        ..Default::default()
    };
    let catalog = map_styles(&styles);
    let s = catalog
        .paragraph_styles
        .get(&StyleId::new("Heading1"))
        .unwrap();
    assert_eq!(s.parent, Some(StyleId::new("Normal")));
}

#[test]
fn character_style_in_catalog() {
    let styles = make_styles(DocxStyleType::Character, "DefaultParagraphFont", "Default");
    let catalog = map_styles(&styles);
    assert!(
        catalog
            .character_styles
            .contains_key(&StyleId::new("DefaultParagraphFont"))
    );
    assert!(
        !catalog
            .paragraph_styles
            .contains_key(&StyleId::new("DefaultParagraphFont"))
    );
}

#[test]
fn table_style_in_table_catalog() {
    let styles = make_styles(DocxStyleType::Table, "TableGrid", "Table Grid");
    let catalog = map_styles(&styles);
    assert!(
        catalog
            .table_styles
            .contains_key(&StyleId::new("TableGrid"))
    );
    assert!(
        !catalog
            .paragraph_styles
            .contains_key(&StyleId::new("TableGrid"))
    );
}

#[test]
fn doc_defaults_create_synthetic_root() {
    use crate::docx::model::paragraph::DocxRPr;
    let styles = DocxStyles {
        default_rpr: Some(DocxRPr {
            bold: Some(true),
            ..Default::default()
        }),
        default_ppr: None,
        styles: vec![],
    };
    let catalog = map_styles(&styles);
    let root = catalog
        .paragraph_styles
        .get(&StyleId::new("__DocDefault"))
        .unwrap();
    assert!(root.is_default);
    assert_eq!(root.char_props.bold, Some(true));
}

#[test]
fn duplicate_style_ids_last_definition_wins() {
    use crate::docx::model::paragraph::DocxRPr;
    use loki_primitives::units::Points;
    let styles = DocxStyles {
        default_rpr: None,
        default_ppr: None,
        styles: vec![
            DocxStyle {
                style_type: DocxStyleType::Paragraph,
                style_id: "Heading1".into(),
                is_default: false,
                is_custom: false,
                name: Some("Heading 1".into()),
                based_on: None,
                next: None,
                link: None,
                ppr: None,
                rpr: Some(DocxRPr {
                    sz: Some(32), // 16pt (half-points)
                    bold: Some(true),
                    ..Default::default()
                }),
            },
            DocxStyle {
                style_type: DocxStyleType::Paragraph,
                style_id: "Heading1".into(),
                is_default: false,
                is_custom: false,
                name: Some("Heading 1".into()),
                based_on: None,
                next: None,
                link: None,
                ppr: None,
                rpr: Some(DocxRPr {
                    sz: Some(36), // 18pt
                    bold: Some(false),
                    ..Default::default()
                }),
            },
        ],
    };
    let catalog = map_styles(&styles);
    let s = catalog
        .paragraph_styles
        .get(&StyleId::new("Heading1"))
        .unwrap();
    assert_eq!(s.char_props.font_size, Some(Points::new(18.0)));
    assert_eq!(s.char_props.bold, Some(false));
}

#[test]
fn missing_normal_style_falls_back_to_doc_defaults() {
    use crate::docx::model::paragraph::{DocxRFonts, DocxRPr};
    use loki_primitives::units::Points;
    let styles = DocxStyles {
        default_rpr: Some(DocxRPr {
            fonts: Some(DocxRFonts {
                ascii: Some("Calibri".into()),
                ..Default::default()
            }),
            sz: Some(22), // 11pt
            ..Default::default()
        }),
        default_ppr: None,
        styles: vec![DocxStyle {
            style_type: DocxStyleType::Paragraph,
            style_id: "Heading1".into(),
            is_default: false,
            is_custom: false,
            name: Some("Heading 1".into()),
            based_on: Some("Normal".into()),
            next: None,
            link: None,
            ppr: None,
            rpr: Some(DocxRPr {
                bold: Some(true),
                ..Default::default()
            }),
        }],
    };
    let catalog = map_styles(&styles);

    // Assert: Normal style was synthesized
    let normal = catalog
        .paragraph_styles
        .get(&StyleId::new("Normal"))
        .unwrap();
    assert!(normal.is_default);

    // Assert: Heading1 inherits font size and family from synthesized Normal style (which inherits from docDefaults)
    let resolved = catalog.resolve_char(&StyleId::new("Heading1")).unwrap();
    assert_eq!(resolved.font_size, Some(Points::new(11.0)));
    assert_eq!(resolved.font_name.as_deref(), Some("Calibri"));
    assert_eq!(resolved.bold, Some(true));
}
