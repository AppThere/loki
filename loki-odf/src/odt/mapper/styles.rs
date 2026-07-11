// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
                next_style_id: None,
                para_props,
                char_props,
                is_default: true,
                is_custom: false,
                extensions: ExtensionBag::default(),
            };
            catalog
                .paragraph_styles
                .insert(StyleId::new("__Default"), style);
        } else if ds.family == OdfStyleFamily::Text {
            // `style:default-style style:family="text"` is the character family's
            // `Default` source (ADR-0012 Decision 1) — the ODF symmetry of OOXML's
            // synthetic `__DocDefaultChar`. Synthesise a `__DefaultChar` character
            // style and point the catalog default at it; a standalone character
            // style then resolves these run defaults as `Provenance::Default`.
            let char_props = ds
                .text_props
                .as_ref()
                .map(map_text_props)
                .unwrap_or_default();
            let id = StyleId::new("__DefaultChar");
            catalog.character_styles.insert(
                id.clone(),
                CharacterStyle {
                    id: id.clone(),
                    display_name: None,
                    parent: None,
                    char_props,
                    extensions: ExtensionBag::default(),
                },
            );
            catalog.default_character_style = Some(id);
        }
    }

    // ── Named and automatic styles ─────────────────────────────────────────
    let all_styles = sheet.named_styles.iter().chain(sheet.auto_styles.iter());

    for s in all_styles {
        let id = StyleId::new(&s.name);
        let parent = s.parent_name.as_deref().map(StyleId::new);
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
                    next_style_id: None,
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
#[path = "styles_tests.rs"]
mod tests;
