// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Styles mapper: [`DocxStyles`] → [`StyleCatalog`].

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::char_style::CharacterStyle;
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::table_style::{TableProps, TableStyle};

use crate::docx::model::styles::{DocxStyleType, DocxStyles};

use super::props::{map_ppr, map_rpr};

/// Translates a [`DocxStyles`] collection into a [`StyleCatalog`].
///
/// Document defaults (`w:docDefaults`) are synthesised as a special
/// `ParagraphStyle` with id `"__DocDefault"` and `is_default = true`;
/// it serves as the root of the inheritance chain.
///
/// Table and numbering styles are mapped minimally (table styles are
/// inserted with default properties; numbering styles are skipped silently).
pub(crate) fn map_styles(styles: &DocxStyles) -> StyleCatalog {
    let mut catalog = StyleCatalog::new();

    // Synthesise a root default style from w:docDefaults.
    if styles.default_ppr.is_some() || styles.default_rpr.is_some() {
        let default_style = ParagraphStyle {
            id: StyleId::new("__DocDefault"),
            display_name: None,
            parent: None,
            linked_char_style: None,
            para_props: styles
                .default_ppr
                .as_ref()
                .map(map_ppr)
                .unwrap_or_default(),
            char_props: styles
                .default_rpr
                .as_ref()
                .map(map_rpr)
                .unwrap_or_default(),
            is_default: true,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };
        catalog
            .paragraph_styles
            .insert(StyleId::new("__DocDefault"), default_style);
    }

    for style in &styles.styles {
        let id = StyleId::new(&style.style_id);
        match style.style_type {
            DocxStyleType::Paragraph => {
                let s = ParagraphStyle {
                    id: id.clone(),
                    display_name: style.name.clone(),
                    parent: style.based_on.as_deref().map(StyleId::new),
                    linked_char_style: style.link.as_deref().map(StyleId::new),
                    para_props: style.ppr.as_ref().map(map_ppr).unwrap_or_default(),
                    char_props: style.rpr.as_ref().map(map_rpr).unwrap_or_default(),
                    is_default: style.is_default,
                    is_custom: false,
                    extensions: ExtensionBag::default(),
                };
                catalog.paragraph_styles.insert(id, s);
            }
            DocxStyleType::Character => {
                let s = CharacterStyle {
                    id: id.clone(),
                    display_name: style.name.clone(),
                    parent: style.based_on.as_deref().map(StyleId::new),
                    char_props: style.rpr.as_ref().map(map_rpr).unwrap_or_default(),
                    extensions: ExtensionBag::default(),
                };
                catalog.character_styles.insert(id, s);
            }
            DocxStyleType::Table => {
                // Map with minimal properties; detailed table-style mapping
                // is deferred to a future session.
                let s = TableStyle {
                    id: id.clone(),
                    display_name: style.name.clone(),
                    parent: style.based_on.as_deref().map(StyleId::new),
                    table_props: TableProps::default(),
                    extensions: ExtensionBag::default(),
                };
                catalog.table_styles.insert(id, s);
            }
            DocxStyleType::Numbering => {
                // Numbering styles are expressed through w:abstractNum/w:num;
                // these are handled by map_numbering in the numbering module.
            }
        }
    }

    catalog
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
        let s = catalog.paragraph_styles.get(&StyleId::new("Normal")).unwrap();
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
        assert!(catalog
            .character_styles
            .contains_key(&StyleId::new("DefaultParagraphFont")));
        assert!(!catalog
            .paragraph_styles
            .contains_key(&StyleId::new("DefaultParagraphFont")));
    }

    #[test]
    fn table_style_in_table_catalog() {
        let styles = make_styles(DocxStyleType::Table, "TableGrid", "Table Grid");
        let catalog = map_styles(&styles);
        assert!(catalog
            .table_styles
            .contains_key(&StyleId::new("TableGrid")));
        assert!(!catalog
            .paragraph_styles
            .contains_key(&StyleId::new("TableGrid")));
    }

    #[test]
    fn doc_defaults_create_synthetic_root() {
        use crate::docx::model::paragraph::DocxRPr;
        let styles = DocxStyles {
            default_rpr: Some(DocxRPr { bold: Some(true), ..Default::default() }),
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
}
