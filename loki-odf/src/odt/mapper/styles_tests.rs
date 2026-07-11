// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the ODT stylesheet mapper (extracted to hold the 300-line ceiling).

use super::*;
use crate::odt::model::styles::{
    OdfDefaultStyle, OdfParaProps, OdfStyle, OdfStyleFamily, OdfStylesheet, OdfTextProps,
};

fn make_para_style(name: &str, parent: Option<&str>, is_auto: bool) -> OdfStyle {
    OdfStyle {
        name: name.into(),
        display_name: None,
        family: OdfStyleFamily::Paragraph,
        parent_name: parent.map(String::from),
        list_style_name: None,
        para_props: None,
        text_props: None,
        col_width: None,
        cell_props: None,
        graphic_wrap: None,
        is_automatic: is_auto,
        master_page_name: None,
    }
}

fn make_text_style(name: &str) -> OdfStyle {
    OdfStyle {
        name: name.into(),
        display_name: Some("Bold Emphasis".into()),
        family: OdfStyleFamily::Text,
        parent_name: None,
        list_style_name: None,
        para_props: None,
        text_props: Some(OdfTextProps {
            font_weight: Some("bold".into()),
            ..Default::default()
        }),
        col_width: None,
        cell_props: None,
        graphic_wrap: None,
        is_automatic: false,
        master_page_name: None,
    }
}

#[test]
fn paragraph_style_inserted() {
    let sheet = OdfStylesheet {
        named_styles: vec![make_para_style("Normal", None, false)],
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    assert!(
        catalog
            .paragraph_styles
            .contains_key(&StyleId::new("Normal"))
    );
}

#[test]
fn character_style_inserted() {
    let sheet = OdfStylesheet {
        named_styles: vec![make_text_style("Strong")],
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    let cs = catalog
        .character_styles
        .get(&StyleId::new("Strong"))
        .unwrap();
    assert_eq!(cs.char_props.bold, Some(true));
}

#[test]
fn parent_is_mapped() {
    let sheet = OdfStylesheet {
        named_styles: vec![
            make_para_style("Normal", None, false),
            make_para_style("Heading1", Some("Normal"), false),
        ],
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    let h1 = catalog
        .paragraph_styles
        .get(&StyleId::new("Heading1"))
        .unwrap();
    assert_eq!(h1.parent, Some(StyleId::new("Normal")));
}

#[test]
fn auto_style_is_custom() {
    let sheet = OdfStylesheet {
        auto_styles: vec![make_para_style("P1", None, true)],
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    let p1 = catalog.paragraph_styles.get(&StyleId::new("P1")).unwrap();
    assert!(p1.is_custom);
}

#[test]
fn default_style_inserted_as_default() {
    use loki_doc_model::style::props::para_props::ParagraphAlignment;
    let sheet = OdfStylesheet {
        default_styles: vec![OdfDefaultStyle {
            family: OdfStyleFamily::Paragraph,
            para_props: Some(OdfParaProps {
                text_align: Some("justify".into()),
                ..Default::default()
            }),
            text_props: None,
        }],
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    let def = catalog
        .paragraph_styles
        .get(&StyleId::new("__Default"))
        .unwrap();
    assert!(def.is_default);
    assert_eq!(def.para_props.alignment, Some(ParagraphAlignment::Justify));
}

#[test]
fn text_default_style_becomes_the_character_default() {
    use loki_doc_model::style::Provenance;
    let sheet = OdfStylesheet {
        default_styles: vec![OdfDefaultStyle {
            family: OdfStyleFamily::Text,
            para_props: None,
            text_props: Some(OdfTextProps {
                font_weight: Some("bold".into()),
                ..Default::default()
            }),
        }],
        ..Default::default()
    };
    let mut catalog = map_stylesheet(&sheet);
    assert_eq!(
        catalog.default_character_style,
        Some(StyleId::new("__DefaultChar")),
    );
    assert_eq!(
        catalog
            .character_styles
            .get(&StyleId::new("__DefaultChar"))
            .unwrap()
            .char_props
            .bold,
        Some(true),
    );
    // A standalone character style now resolves the ODF text default as
    // `Provenance::Default` (the character family's `Default` source).
    catalog.character_styles.insert(
        StyleId::new("Plain"),
        CharacterStyle {
            id: StyleId::new("Plain"),
            display_name: Some("Plain".into()),
            parent: None,
            char_props: Default::default(),
            extensions: ExtensionBag::default(),
        },
    );
    let r = catalog
        .resolve_char_chain(&StyleId::new("Plain"), |s| s.char_props.bold)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Default);
    assert_eq!(r.value, Some(true));
}

#[test]
fn unknown_family_skipped() {
    let sheet = OdfStylesheet {
        named_styles: vec![OdfStyle {
            name: "T1".into(),
            display_name: None,
            family: OdfStyleFamily::Table,
            parent_name: None,
            list_style_name: None,
            para_props: None,
            text_props: None,
            col_width: None,
            cell_props: None,
            graphic_wrap: None,
            is_automatic: false,
            master_page_name: None,
        }],
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    assert!(catalog.paragraph_styles.is_empty());
    assert!(catalog.character_styles.is_empty());
}

#[test]
fn insertion_order_preserved() {
    let names = ["Alpha", "Beta", "Gamma", "Delta"];
    let styles: Vec<_> = names
        .iter()
        .map(|n| make_para_style(n, None, false))
        .collect();
    let sheet = OdfStylesheet {
        named_styles: styles,
        ..Default::default()
    };
    let catalog = map_stylesheet(&sheet);
    let keys: Vec<_> = catalog.paragraph_styles.keys().collect();
    assert_eq!(keys.len(), 4);
    for (i, name) in names.iter().enumerate() {
        assert_eq!(keys[i].as_str(), *name);
    }
}
