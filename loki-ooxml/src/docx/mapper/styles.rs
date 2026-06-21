// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Styles mapper: [`DocxStyles`] → [`StyleCatalog`].

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::char_style::CharacterStyle;
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
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
            next_style_id: None,
            para_props: styles.default_ppr.as_ref().map(map_ppr).unwrap_or_default(),
            char_props: styles.default_rpr.as_ref().map(map_rpr).unwrap_or_default(),
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
                    next_style_id: style.next.clone(),
                    para_props: style.ppr.as_ref().map(map_ppr).unwrap_or_default(),
                    char_props: style.rpr.as_ref().map(map_rpr).unwrap_or_default(),
                    is_default: style.is_default,
                    is_custom: style.is_custom,
                    extensions: ExtensionBag::default(),
                };
                // COMPAT(microsoft): duplicate styleId — last definition wins,
                // matching Microsoft Word's behavior per §2.7.3.17.
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
                // COMPAT(microsoft): duplicate styleId — last definition wins,
                // matching Microsoft Word's behavior per §2.7.3.17.
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
                // COMPAT(microsoft): duplicate styleId — last definition wins,
                // matching Microsoft Word's behavior per §2.7.3.17.
                catalog.table_styles.insert(id, s);
            }
            DocxStyleType::Numbering => {
                // Numbering styles are expressed through w:abstractNum/w:num;
                // these are handled by map_numbering in the numbering module.
            }
        }
    }

    // COMPAT(microsoft): Normal style missing from styles.xml —
    // synthesize from docDefaults per OOXML §2.7.3.
    // This is common in programmatically generated documents.
    // Note: If the styles part is completely missing (empty), we should not synthesize it.
    let has_any_defined_style =
        !styles.styles.is_empty() || styles.default_ppr.is_some() || styles.default_rpr.is_some();
    if has_any_defined_style
        && !catalog
            .paragraph_styles
            .contains_key(&StyleId::new("Normal"))
    {
        let parent = if catalog
            .paragraph_styles
            .contains_key(&StyleId::new("__DocDefault"))
        {
            Some(StyleId::new("__DocDefault"))
        } else {
            None
        };
        let normal_style = ParagraphStyle {
            id: StyleId::new("Normal"),
            display_name: Some("Normal".into()),
            parent,
            linked_char_style: None,
            next_style_id: None,
            para_props: ParaProps::default(),
            char_props: CharProps::default(),
            is_default: true,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };
        catalog
            .paragraph_styles
            .insert(StyleId::new("Normal"), normal_style);
    }

    catalog
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "styles_tests.rs"]
mod tests;
