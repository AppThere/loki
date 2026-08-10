// SPDX-License-Identifier: Apache-2.0

//! The paper preview of the table about to be inserted (design note 24).
//!
//! The header rule is **drawn, not described**: whether the first row is a
//! header is the one choice in this dialog with a consequence a user cannot
//! infer from the checkbox label, and it is visible here the moment they toggle
//! it.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::spec::TableSpec;

/// Rows of specimen content the preview draws before it elides.
const PREVIEW_ROWS: usize = 3;
/// Columns the preview draws before it elides.
const PREVIEW_COLS: usize = 4;

/// The paper specimen.
pub(super) fn specimen(spec: &TableSpec) -> Element {
    let spec = spec.clamped();
    let cols = spec.cols.min(PREVIEW_COLS);
    let body_rows = spec.body_rows().min(PREVIEW_ROWS);
    let elided = spec.cols > cols || spec.body_rows() > body_rows;
    let caption = spec.caption.trim().to_string();

    rsx! {
        div {
            style: format!(
                "background: {bg}; color: {ink}; border-radius: {r}px; \
                 padding: {p}px; font-family: {serif}; font-size: {fs}px;",
                bg = tokens::CANVAS_PAGE_BG,
                ink = tokens::COLOR_TEXT_PRIMARY,
                r = tokens::RADIUS_MD,
                p = tokens::SPACE_4,
                serif = "Tinos, Gelasio, serif",
                fs = tokens::FONT_SIZE_META,
            ),

            if !caption.is_empty() {
                div {
                    style: format!(
                        "margin-bottom: {mb}px; font-family: {ui}; font-size: {fs}px; color: {fg};",
                        mb = tokens::SPACE_2,
                        ui = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_XS,
                        fg = tokens::COLOR_TEXT_SECONDARY,
                    ),
                    { fl!("table-dialog-preview-caption", caption = caption) }
                }
            }

            if spec.header_row {
                div {
                    style: row_style(true),
                    for c in 0..cols {
                        div {
                            key: "h-{c}",
                            style: cell_style(true),
                            { fl!("table-dialog-preview-header-cell", index = (c + 1) as i64) }
                        }
                    }
                }
            }

            for r in 0..body_rows {
                div {
                    key: "r-{r}",
                    style: row_style(false),
                    for c in 0..cols {
                        div { key: "c-{r}-{c}", style: cell_style(false), "\u{2014}" }
                    }
                }
            }

            if elided {
                div {
                    style: format!(
                        "margin-top: {mt}px; font-family: {ui}; font-size: {fs}px; color: {fg};",
                        mt = tokens::SPACE_2,
                        ui = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_XS,
                        fg = tokens::COLOR_TEXT_SECONDARY,
                    ),
                    {
                        fl!(
                            "table-dialog-preview-elided",
                            rows = spec.rows as i64,
                            cols = spec.cols as i64
                        )
                    }
                }
            }
        }
    }
}

/// A specimen row. The header carries the heavier rule beneath it.
fn row_style(header: bool) -> String {
    format!(
        "display: flex; flex-direction: row; border-bottom: {w} solid {color};",
        w = if header { "1.5px" } else { "1px" },
        color = if header {
            tokens::COLOR_TEXT_PRIMARY
        } else {
            tokens::COLOR_BORDER_DEFAULT
        },
    )
}

/// A specimen cell.
fn cell_style(header: bool) -> String {
    format!(
        "flex: 1; min-width: 0; padding: {py}px {px}px; font-weight: {fw}; color: {fg};",
        py = tokens::SPACE_1,
        px = tokens::SPACE_2,
        fw = if header {
            tokens::FONT_WEIGHT_BOLD
        } else {
            tokens::FONT_WEIGHT_REGULAR
        },
        fg = if header {
            tokens::COLOR_TEXT_PRIMARY
        } else {
            tokens::COLOR_TEXT_SECONDARY
        },
    )
}
