// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use crate::docx::model::styles::{DocxStyle, DocxStyleType, DocxTableStyleProps, DocxTblStylePr};

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
            table: None,
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
            table: None,
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
    // Not flagged default → no default table style recorded.
    assert_eq!(catalog.default_table_style, None);
}

#[test]
fn default_flagged_table_style_becomes_the_table_default() {
    let styles = DocxStyles {
        default_rpr: None,
        default_ppr: None,
        styles: vec![DocxStyle {
            style_type: DocxStyleType::Table,
            style_id: "TableNormal".into(),
            is_default: true,
            is_custom: false,
            name: Some("Table Normal".into()),
            based_on: None,
            next: None,
            link: None,
            ppr: None,
            rpr: None,
            table: None,
        }],
    };
    let catalog = map_styles(&styles);
    assert_eq!(
        catalog.default_table_style,
        Some(StyleId::new("TableNormal")),
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
fn doc_defaults_create_the_default_character_style() {
    use crate::docx::model::paragraph::DocxRPr;
    // The same `w:rPrDefault` also synthesises the character family's `Default`
    // source (ADR-0012 Decision 1), pointed at by `default_character_style`.
    let styles = DocxStyles {
        default_rpr: Some(DocxRPr {
            bold: Some(true),
            ..Default::default()
        }),
        default_ppr: None,
        styles: vec![],
    };
    let catalog = map_styles(&styles);
    assert_eq!(
        catalog.default_character_style,
        Some(StyleId::new("__DocDefaultChar")),
    );
    let def = catalog
        .character_styles
        .get(&StyleId::new("__DocDefaultChar"))
        .expect("synthetic default character style present");
    assert_eq!(def.char_props.bold, Some(true));
    // A standalone character style with `bold` unset now resolves the docDefault
    // as `Default` (was `FormatDefault` before this source existed).
    let mut cat = catalog;
    cat.character_styles.insert(
        StyleId::new("Plain"),
        CharacterStyle {
            id: StyleId::new("Plain"),
            display_name: Some("Plain".into()),
            parent: None,
            char_props: CharProps::default(),
            extensions: ExtensionBag::default(),
        },
    );
    let r = cat
        .resolve_char_chain(&StyleId::new("Plain"), |s| s.char_props.bold)
        .unwrap();
    assert_eq!(r.provenance, loki_doc_model::style::Provenance::Default);
    assert_eq!(r.value, Some(true));
}

#[test]
fn no_doc_defaults_leaves_no_default_character_style() {
    let styles = DocxStyles {
        default_rpr: None,
        default_ppr: None,
        styles: vec![],
    };
    let catalog = map_styles(&styles);
    assert_eq!(catalog.default_character_style, None);
    assert!(
        !catalog
            .character_styles
            .contains_key(&StyleId::new("__DocDefaultChar"))
    );
}

#[test]
fn default_paragraph_style_resolves_doc_default_font() {
    use crate::docx::model::paragraph::{DocxRFonts, DocxRPr};
    // docDefaults font Calibri, no explicit pStyle → Normal synthesized.
    let styles = DocxStyles {
        default_rpr: Some(DocxRPr {
            fonts: Some(DocxRFonts {
                ascii: Some("Calibri".into()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        default_ppr: None,
        styles: vec![],
    };
    let catalog = map_styles(&styles);

    // A bare paragraph (no w:pStyle) must resolve through the recorded default
    // paragraph style, which inherits the docDefaults font.
    let def = catalog
        .default_paragraph_style
        .clone()
        .expect("default paragraph style recorded");
    assert_eq!(def, StyleId::new("Normal"));
    let resolved = catalog
        .effective_paragraph_style(None)
        .and_then(|id| catalog.resolve_char(id))
        .expect("bare paragraph resolves the default style");
    assert_eq!(resolved.font_name.as_deref(), Some("Calibri"));
}

#[test]
fn explicit_default_paragraph_style_is_preferred() {
    // A paragraph style flagged w:default="1" wins over the synthesized Normal.
    let styles = DocxStyles {
        default_rpr: None,
        default_ppr: None,
        styles: vec![DocxStyle {
            style_type: DocxStyleType::Paragraph,
            style_id: "MyBody".into(),
            is_default: true,
            is_custom: false,
            name: Some("My Body".into()),
            based_on: None,
            next: None,
            link: None,
            ppr: None,
            rpr: None,
            table: None,
        }],
    };
    let catalog = map_styles(&styles);
    assert_eq!(
        catalog.default_paragraph_style,
        Some(StyleId::new("MyBody"))
    );
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
                table: None,
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
                table: None,
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
            table: None,
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

/// A table style with band sizes, base shading, and `w:tblStylePr` regions maps
/// into `TableStyle.table_props` + the `conditional` region map; unknown region
/// names and unshaded regions are skipped.
#[test]
fn table_style_conditional_formatting_maps() {
    use loki_doc_model::style::table_style::TableRegion;

    let table = DocxTableStyleProps {
        row_band_size: Some(2),
        col_band_size: None,
        base_shd_fill: Some("FFFFFF".into()),
        conditional: vec![
            DocxTblStylePr {
                region: "firstRow".into(),
                shd_fill: Some("4472C4".into()),
            },
            DocxTblStylePr {
                region: "band1Horz".into(),
                shd_fill: Some("D9E2F3".into()),
            },
            // Unknown region → skipped.
            DocxTblStylePr {
                region: "bogusRegion".into(),
                shd_fill: Some("000000".into()),
            },
            // Known region but no shading → skipped.
            DocxTblStylePr {
                region: "lastRow".into(),
                shd_fill: None,
            },
        ],
    };
    let styles = DocxStyles {
        default_rpr: None,
        default_ppr: None,
        styles: vec![DocxStyle {
            style_type: DocxStyleType::Table,
            style_id: "Banded".into(),
            is_default: false,
            is_custom: false,
            name: Some("Banded".into()),
            based_on: None,
            next: None,
            link: None,
            ppr: None,
            rpr: None,
            table: Some(table),
        }],
    };

    let catalog = map_styles(&styles);
    let ts = catalog
        .table_styles
        .get(&StyleId::new("Banded"))
        .expect("table style present");

    assert_eq!(ts.table_props.row_band_size, Some(2));
    assert_eq!(ts.table_props.col_band_size, None);
    assert!(ts.table_props.background_color.is_some());
    // Only the two shaded, known regions survive.
    assert_eq!(ts.conditional.len(), 2);
    assert!(ts.conditional.contains_key(&TableRegion::FirstRow));
    assert!(ts.conditional.contains_key(&TableRegion::Band1Horz));
    assert!(!ts.conditional.contains_key(&TableRegion::LastRow));
}
