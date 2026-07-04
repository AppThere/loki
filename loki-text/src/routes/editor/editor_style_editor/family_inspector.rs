// SPDX-License-Identifier: Apache-2.0

//! The right-hand read-only inspector columns for the non-paragraph families
//! (Spec 05 M6 §9). Rendered to the right of the paragraph provenance column
//! when a character or list style is selected in the left column.
//!
//! Factored out of `mod.rs` so the panel body stays under the ceiling and both
//! family columns share one boundary. The character column reuses
//! [`super::provenance::CharRowsSection`]; the list column is rendered here
//! because list styles are non-inheriting (per-level rows, no provenance chip).

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::panel_data::{CharSelection, ListSelection};
use super::provenance::CharRowsSection;

/// Renders the character and list read-only inspector columns, each present only
/// when its family has a selection. Both sit to the right of the paragraph
/// provenance column.
pub(super) fn family_inspector_columns(
    char_selected_rows: CharSelection,
    list_selected_rows: ListSelection,
) -> Element {
    rsx! {
        // ── Character inspector (read-only; §9 character family) ────────────
        if let Some((name, rows)) = char_selected_rows {
            div { style: column_style(), CharRowsSection { heading: name, rows } }
        }

        // ── List inspector (read-only; §9 list family, non-inheriting) ──────
        if let Some((name, rows)) = list_selected_rows {
            div {
                style: column_style(),
                div {
                    style: format!(
                        "font-size: {fs}px; font-weight: {fw}; color: {fg}; margin-bottom: {mb}px;",
                        fs = tokens::FONT_SIZE_LABEL,
                        fw = tokens::FONT_WEIGHT_MEDIUM,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        mb = tokens::SPACE_1,
                    ),
                    { name }
                }
                for lvl in rows.into_iter() {
                    div {
                        key: "list-level-{lvl.level}",
                        style: format!("display: flex; flex-direction: column; margin-bottom: {}px;", tokens::SPACE_1),
                        span {
                            style: format!(
                                "font-size: {fs}px; color: {fg}; font-weight: {fw};",
                                fs = tokens::FONT_SIZE_LABEL,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                                fw = tokens::FONT_WEIGHT_SEMIBOLD,
                            ),
                            { fl!("style-list-level-label", n = lvl.level as i64) }
                        }
                        span {
                            style: format!(
                                "font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_XS,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            {
                                fl!(
                                    "style-list-level-detail",
                                    label = lvl.label,
                                    indent = lvl.indent,
                                    hanging = lvl.hanging,
                                    alignment = lvl.alignment
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The shared 220px read-only column chrome (border + padding + scroll).
fn column_style() -> String {
    format!(
        "width: 220px; min-width: 220px; overflow-y: auto; \
         border-left: 1px solid {border}; padding: {p}px;",
        border = tokens::COLOR_BORDER_CHROME,
        p = tokens::SPACE_3,
    )
}
