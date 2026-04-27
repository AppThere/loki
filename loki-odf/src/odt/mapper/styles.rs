// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Stylesheet mapper: converts an [`OdfStylesheet`] into a
//! format-neutral [`StyleCatalog`].

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::char_style::CharacterStyle;
use loki_doc_model::style::para_style::ParagraphStyle;

use crate::odt::mapper::props::{map_para_props, map_text_props};
use crate::odt::model::styles::{OdfStyleFamily, OdfStylesheet};

/// Convert an [`OdfStylesheet`] into a format-neutral [`StyleCatalog`].
///
/// Walks `default_styles`, `named_styles`, and `auto_styles` in that order.
/// Default paragraph styles are inserted under `StyleId("__Default")` with
/// `is_default = true`. Named and automatic styles are keyed by their ODF
/// `style:name`.
///
/// - `OdfStyleFamily::Paragraph` → [`ParagraphStyle`]
/// - `OdfStyleFamily::Text` → [`CharacterStyle`]
/// - All other families are skipped.
pub(crate) fn map_stylesheet(sheet: &OdfStylesheet) -> StyleCatalog {
    let mut catalog = StyleCatalog::new();

    // ── Default styles ─────────────────────────────────────────────────────
    for ds in &sheet.default_styles {
        if ds.family == OdfStyleFamily::Paragraph {
            let para_props = ds
                .para_props
                .as_ref()
                .map(map_para_props)
                .unwrap_or_default();
            let char_props = ds
                .text_props
                .as_ref()
                .map(map_text_props)
                .unwrap_or_default();
            let style = ParagraphStyle {
                id: StyleId::new("__Default"),
                display_name: None,
                parent: None,
                linked_char_style: None,
                para_props,
                char_props,
                is_default: true,
                is_custom: false,
                extensions: ExtensionBag::default(),
            };
            catalog
                .paragraph_styles
                .insert(StyleId::new("__Default"), style);
        }
    }

    // ── Named and automatic styles ─────────────────────────────────────────
    let all_styles =
        sheet.named_styles.iter().chain(sheet.auto_styles.iter());

    for s in all_styles {
        let id = StyleId::new(&s.name);
        let parent = s
            .parent_name
            .as_deref()
            .map(StyleId::new);
        let display_name = s.display_name.clone();
        let is_custom = s.is_automatic;

        match s.family {
            OdfStyleFamily::Paragraph => {
                let para_props = s
                    .para_props
                    .as_ref()
                    .map(map_para_props)
                    .unwrap_or_default();
                let char_props = s
                    .text_props
                    .as_ref()
                    .map(map_text_props)
                    .unwrap_or_default();

                // Build linked char style id from list_style_name if present
                // (ODF uses text:list-style-name on paragraph styles, not a
                // linked char style; leave linked_char_style as None here).
                let style = ParagraphStyle {
                    id: id.clone(),
                    display_name,
                    parent,
                    linked_char_style: None,
                    para_props,
                    char_props,
                    is_default: false,
                    is_custom,
                    extensions: ExtensionBag::default(),
                };
                catalog.paragraph_styles.insert(id, style);
            }
            OdfStyleFamily::Text => {
                let char_props = s
                    .text_props
                    .as_ref()
                    .map(map_text_props)
                    .unwrap_or_default();
                let style = CharacterStyle {
                    id: id.clone(),
                    display_name,
                    parent,
                    char_props,
                    extensions: ExtensionBag::default(),
                };
                catalog.character_styles.insert(id, style);
            }
            // Table, graphic, and unknown families are not mapped here
            _ => {}
        }
    }

    catalog
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::odt::model::styles::{
        OdfDefaultStyle, OdfStyle, OdfStyleFamily, OdfStylesheet, OdfTextProps,
        OdfParaProps,
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
            is_automatic: is_auto,
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
            is_automatic: false,
        }
    }

    #[test]
    fn paragraph_style_inserted() {
        let sheet = OdfStylesheet {
            named_styles: vec![make_para_style("Normal", None, false)],
            ..Default::default()
        };
        let catalog = map_stylesheet(&sheet);
        assert!(catalog.paragraph_styles.contains_key(&StyleId::new("Normal")));
    }

    #[test]
    fn character_style_inserted() {
        let sheet = OdfStylesheet {
            named_styles: vec![make_text_style("Strong")],
            ..Default::default()
        };
        let catalog = map_stylesheet(&sheet);
        let cs = catalog.character_styles.get(&StyleId::new("Strong")).unwrap();
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
        let h1 = catalog.paragraph_styles.get(&StyleId::new("Heading1")).unwrap();
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
        use loki_doc_model::style::props::para_props::ParagraphAlignment;
        assert_eq!(def.para_props.alignment, Some(ParagraphAlignment::Justify));
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
                is_automatic: false,
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
}
