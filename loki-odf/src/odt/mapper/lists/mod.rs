// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List-style mapper: converts [`OdfListStyle`]s into
//! format-neutral [`ListStyle`]s and inserts them into a [`StyleCatalog`].

mod indentation;
mod kind;

#[cfg(test)]
mod tests;

use indentation::map_indentation;
use kind::map_level_kind;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{LabelAlignment, ListId, ListLevel, ListStyle};

use crate::odt::model::list_styles::OdfListStyle;
use crate::version::OdfVersion;
use crate::xml_util::parse_length;

/// Convert all list styles in `sheet` and insert them into `catalog`.
///
/// Indentation is mapped using either the ODF 1.2+ label-alignment model
/// (when [`OdfVersion::supports_label_alignment`] returns `true` and
/// `label_followed_by` is `Some`) or the legacy ODF 1.1 model.
pub(crate) fn map_list_styles(
    list_styles: &[OdfListStyle],
    catalog: &mut StyleCatalog,
    version: OdfVersion,
) {
    for odf_ls in list_styles {
        let id = ListId::new(&odf_ls.name);
        let mut levels: Vec<ListLevel> = Vec::new();

        for odf_level in &odf_ls.levels {
            let level_num = odf_level.level + 1; // 0-indexed → 1-indexed ODF

            let kind = map_level_kind(&odf_level.kind, level_num);

            let (indent_start, hanging_indent) = map_indentation(odf_level, version);

            // Label char props from ODF text props on the level element
            let char_props = odf_level
                .text_props
                .as_ref()
                .map(crate::odt::mapper::props::map_text_props)
                .unwrap_or_default();

            levels.push(ListLevel {
                level: odf_level.level,
                kind,
                indent_start,
                hanging_indent,
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: odf_level
                    .list_tab_stop_position
                    .as_deref()
                    .and_then(parse_length),
                char_props,
            });
        }

        catalog.list_styles.insert(
            id.clone(),
            ListStyle {
                id,
                display_name: None,
                levels,
                extensions: ExtensionBag::default(),
            },
        );
    }
}
