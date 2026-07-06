// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! One row of [`super::recent_files::AtRecentFileList`]: the document info
//! button, the ⋮ menu toggle, and the inline context menu.
//!
//! A `#[component]` (not a loop body) so it owns its hook scope — the hover
//! signal used to be a `use_signal` inside the list's `for` loop, which made
//! the parent's hook count depend on the document count (audit F6a /
//! ADR-0013).

use dioxus::prelude::*;

use crate::tokens::colors::{
    COLOR_STATUS_ERROR_TEXT, COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME_SECONDARY,
    COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};

/// Props for [`RecentRow`]. The parent owns the open-menu state; the row
/// reports toggle/action clicks by index.
#[derive(Props, Clone, PartialEq)]
pub(super) struct RecentRowProps {
    pub idx: usize,
    pub title: String,
    pub modified: String,
    pub is_menu_open: bool,
    pub menu_aria_label: String,
    pub remove_label: String,
    pub delete_label: String,
    pub open_copy_label: String,
    pub on_select: EventHandler<usize>,
    pub on_toggle_menu: EventHandler<usize>,
    pub on_remove: EventHandler<usize>,
    pub on_delete: EventHandler<usize>,
    pub on_open_copy: EventHandler<usize>,
}

/// A recent-document row with its inline context menu.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
/// The row click target, the ⋮ button, and every menu action meet this.
#[component]
pub(super) fn RecentRow(props: RecentRowProps) -> Element {
    let mut row_hovered = use_signal(|| false);
    let row_bg = if row_hovered() {
        "#F5F5F5"
    } else {
        COLOR_SURFACE_PAGE
    };
    let idx = props.idx;
    // Shared base style for context-menu action buttons; caller supplies `fg`.
    let action_style = |fg: &'static str| {
        format!(
            "background: transparent; border: none; text-align: left; \
             padding: {p}px {ph}px; min-height: {touch}px; cursor: pointer; \
             font-size: {size}px; color: {fg}; border-radius: {r}px;",
            p = SPACE_2,
            ph = SPACE_3,
            touch = TOUCH_MIN,
            size = FONT_SIZE_BODY,
            fg = fg,
            r = RADIUS_SM,
        )
    };

    rsx! {
        div {
            style: format!(
                "background: {bg}; border-radius: {r}px;",
                bg = row_bg,
                r  = RADIUS_MD,
            ),
            onmouseenter: move |_| { row_hovered.set(true); },
            onmouseleave: move |_| { row_hovered.set(false); },

            // ── Row: document info button + ⋮ toggle ──────────────────────────
            div {
                style: "display: flex; align-items: center;",
                button {
                    "aria-label": props.title.clone(),
                    style: format!(
                        "background: transparent; border: none; \
                         border-radius: {r}px; \
                         padding: {pv}px {ph}px; min-height: {touch}px; \
                         flex: 1; display: flex; flex-direction: column; \
                         gap: {gap}px; cursor: pointer; \
                         text-align: left; box-sizing: border-box;",
                        r     = RADIUS_MD,
                        pv    = SPACE_3,
                        ph    = SPACE_4,
                        touch = TOUCH_MIN,
                        gap   = SPACE_1,
                    ),
                    onclick: move |_| { props.on_select.call(idx); },
                    span {
                        style: format!(
                            "font-size: {size}px; font-weight: {weight}; color: {fg};",
                            size   = FONT_SIZE_BODY,
                            weight = FONT_WEIGHT_SEMIBOLD,
                            fg     = COLOR_TEXT_PRIMARY,
                        ),
                        "{props.title}"
                    }
                    span {
                        style: format!(
                            "font-size: {size}px; color: {fg};",
                            size = FONT_SIZE_LABEL,
                            fg   = COLOR_TEXT_SECONDARY,
                        ),
                        "{props.modified}"
                    }
                }
                // ── ⋮ context menu button ─────────────────────────────────────
                button {
                    "aria-label":    props.menu_aria_label.clone(),
                    "aria-expanded": if props.is_menu_open { "true" } else { "false" },
                    style: format!(
                        "background: transparent; border: none; \
                         min-width: {touch}px; min-height: {touch}px; \
                         border-radius: {r}px; cursor: pointer; \
                         font-size: 18px; color: {fg}; flex-shrink: 0;",
                        touch = TOUCH_MIN,
                        r     = RADIUS_SM,
                        fg    = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    onclick: move |_| { props.on_toggle_menu.call(idx); },
                    "⋮"
                }
            }

            // ── Inline context menu ───────────────────────────────────────────
            if props.is_menu_open {
                div {
                    style: format!(
                        "display: flex; flex-direction: column; \
                         border-top: 1px solid #E0E0E0; \
                         padding: {p}px; gap: {gap}px;",
                        p   = SPACE_2,
                        gap = SPACE_1,
                    ),
                    button {
                        style: action_style(COLOR_TEXT_PRIMARY),
                        onclick: move |_| { props.on_remove.call(idx); },
                        "{props.remove_label}"
                    }
                    button {
                        style: action_style(COLOR_STATUS_ERROR_TEXT),
                        onclick: move |_| { props.on_delete.call(idx); },
                        "{props.delete_label}"
                    }
                    button {
                        style: action_style(COLOR_TEXT_PRIMARY),
                        onclick: move |_| { props.on_open_copy.call(idx); },
                        "{props.open_copy_label}"
                    }
                }
            }
        }
    }
}
