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

use super::panel_data::{CharSelection, ListSelection, PageSelection};
use super::posture::StylePanelPosture;
use super::provenance::CharRowsSection;

/// Renders the character, list, and page read-only inspector columns, each
/// present only when its family has a selection. All sit to the right of the
/// paragraph provenance column (or stack full-width beneath it at Compact).
pub(super) fn family_inspector_columns(
    char_selected_rows: CharSelection,
    list_selected_rows: ListSelection,
    page_selected_rows: PageSelection,
    posture: StylePanelPosture,
) -> Element {
    rsx! {
        // ── Character inspector (read-only; §9 character family) ────────────
        if let Some((name, rows)) = char_selected_rows {
            div { style: column_style(posture), CharRowsSection { heading: name, rows } }
        }

        // ── List inspector (read-only; §9 list family, non-inheriting) ──────
        if let Some((name, rows)) = list_selected_rows {
            div {
                style: column_style(posture),
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

        // ── Page inspector (read-only; §9 page family, non-inheriting) ──────
        if let Some((name, rows)) = page_selected_rows {
            div {
                style: column_style(posture),
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
                for row in rows.into_iter() {
                    div {
                        key: "page-{row.label_key}",
                        style: format!("display: flex; flex-direction: row; justify-content: space-between; gap: {}px; margin-bottom: 2px;", tokens::SPACE_2),
                        span {
                            style: format!(
                                "font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_LABEL,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            { fl!(row.label_key) }
                        }
                        span {
                            style: format!(
                                "font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_LABEL,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            { row.value }
                        }
                    }
                }
            }
        }
    }
}

/// The shared read-only column chrome (border + padding + scroll). 220px wide at
/// Expanded; full-width when the panel stacks at Compact.
fn column_style(posture: StylePanelPosture) -> String {
    format!(
        "width: {w}; min-width: {w}; overflow-y: auto; \
         border-left: 1px solid {border}; padding: {p}px;",
        w = posture.section_width(220.0),
        border = tokens::COLOR_BORDER_CHROME,
        p = tokens::SPACE_3,
    )
}
