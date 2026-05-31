// SPDX-License-Identifier: Apache-2.0

//! `AtRecentFileList` — recent documents list with per-row context menu.

use dioxus::prelude::*;

use crate::components::home_tab::RecentDocument;
use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_ACCENT_PRIMARY_HOVER, COLOR_STATUS_ERROR_TEXT, COLOR_SURFACE_PAGE,
    COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD,
};

/// Maximum number of recent entries displayed in the list.
const RECENT_VISIBLE_LIMIT: usize = 10;

// ── AtRecentFileList ──────────────────────────────────────────────────────────

/// Vertically scrollable list of recently opened documents.
///
/// Each row has a primary click area (opens the document) and a ⋮ button
/// that expands an inline context menu with document management actions.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
/// Both the row click target and the ⋮ button meet this requirement.
#[component]
pub(crate) fn AtRecentFileList(props: AtRecentFileListProps) -> Element {
    let mut menu_open: Signal<Option<usize>> = use_signal(|| None);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; \
                 font-family: {font};",
                gap  = SPACE_2,
                font = FONT_FAMILY_UI,
            ),

            if props.documents.is_empty() {
                // ── Empty state ───────────────────────────────────────────────
                {
                    let mut open_hovered = use_signal(|| false);
                    let open_bg = if open_hovered() {
                        COLOR_ACCENT_PRIMARY_HOVER
                    } else {
                        COLOR_ACCENT_PRIMARY
                    };
                    rsx! {
                        div {
                            style: format!(
                                "display: flex; flex-direction: column; \
                                 align-items: center; gap: {gap}px; padding: {p}px;",
                                gap = SPACE_4,
                                p   = SPACE_4,
                            ),
                            span {
                                style: format!(
                                    "font-size: {size}px; color: {fg};",
                                    size = FONT_SIZE_BODY,
                                    fg   = COLOR_TEXT_ON_CHROME_SECONDARY,
                                ),
                                "{props.empty_label}"
                            }
                            button {
                                style: format!(
                                    "background: {bg}; color: {fg}; \
                                     border: none; border-radius: {r}px; \
                                     min-height: {touch}px; padding: 0 {px}px; \
                                     font-size: {size}px; font-weight: {weight}; \
                                     cursor: pointer;",
                                    bg     = open_bg,
                                    fg     = COLOR_SURFACE_PAGE,
                                    r      = RADIUS_SM,
                                    touch  = TOUCH_MIN,
                                    px     = SPACE_4,
                                    size   = FONT_SIZE_BODY,
                                    weight = FONT_WEIGHT_SEMIBOLD,
                                ),
                                onmouseenter: move |_| { open_hovered.set(true); },
                                onmouseleave: move |_| { open_hovered.set(false); },
                                onclick: move |_| { props.on_open_file.call(()); },
                                "{props.open_file_label}"
                            }
                        }
                    }
                }
            }

            for (idx, doc) in props.documents.iter().take(RECENT_VISIBLE_LIMIT).enumerate() {
                {
                    let title    = doc.title.clone();
                    let path     = doc.path.clone();
                    let modified = doc.modified_at.clone();
                    let mut row_hovered = use_signal(|| false);
                    let row_bg = if row_hovered() { "#F5F5F5" } else { COLOR_SURFACE_PAGE };
                    let is_menu_open = menu_open() == Some(idx);
                    // Shared base style for context-menu action buttons; caller supplies `color`.
                    let action_style = |fg: &'static str| format!(
                        "background: transparent; border: none; text-align: left; \
                         padding: {p}px {ph}px; min-height: {touch}px; cursor: pointer; \
                         font-size: {size}px; color: {fg}; border-radius: {r}px;",
                        p = SPACE_2, ph = SPACE_3, touch = TOUCH_MIN,
                        size = FONT_SIZE_BODY, fg = fg, r = RADIUS_SM,
                    );
                    rsx! {
                        div {
                            key: "{idx}",
                            style: format!(
                                "background: {bg}; border-radius: {r}px;",
                                bg = row_bg,
                                r  = RADIUS_MD,
                            ),
                            onmouseenter: move |_| { row_hovered.set(true); },
                            onmouseleave: move |_| { row_hovered.set(false); },

                            // ── Row: document info button + ⋮ toggle ──────────
                            div {
                                style: "display: flex; align-items: center;",
                                button {
                                    "aria-label": title.clone(),
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
                                        "{title}"
                                    }
                                    span {
                                        style: format!(
                                            "font-size: {size}px; color: {fg};",
                                            size = FONT_SIZE_LABEL,
                                            fg   = COLOR_TEXT_SECONDARY,
                                        ),
                                        "{path}"
                                    }
                                    span {
                                        style: format!(
                                            "font-size: {size}px; color: {fg};",
                                            size = FONT_SIZE_LABEL,
                                            fg   = COLOR_TEXT_SECONDARY,
                                        ),
                                        "{modified}"
                                    }
                                }
                                // ── ⋮ context menu button ─────────────────────
                                button {
                                    "aria-label":    props.menu_aria_label.clone(),
                                    "aria-expanded": if is_menu_open { "true" } else { "false" },
                                    style: format!(
                                        "background: transparent; border: none; \
                                         min-width: {touch}px; min-height: {touch}px; \
                                         border-radius: {r}px; cursor: pointer; \
                                         font-size: 18px; color: {fg}; flex-shrink: 0;",
                                        touch = TOUCH_MIN,
                                        r     = RADIUS_SM,
                                        fg    = COLOR_TEXT_ON_CHROME_SECONDARY,
                                    ),
                                    onclick: move |_| {
                                        menu_open.set(if is_menu_open { None } else { Some(idx) });
                                    },
                                    "⋮"
                                }
                            }

                            // ── Inline context menu ───────────────────────────
                            if is_menu_open {
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
                                        onclick: move |_| {
                                            menu_open.set(None);
                                            props.on_remove.call(idx);
                                        },
                                        "{props.remove_label}"
                                    }
                                    button {
                                        style: action_style(COLOR_STATUS_ERROR_TEXT),
                                        onclick: move |_| {
                                            menu_open.set(None);
                                            props.on_delete.call(idx);
                                        },
                                        "{props.delete_label}"
                                    }
                                    button {
                                        style: action_style(COLOR_TEXT_PRIMARY),
                                        onclick: move |_| {
                                            menu_open.set(None);
                                            props.on_open_copy.call(idx);
                                        },
                                        "{props.open_copy_label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Open File button shown below the list when documents exist
            if !props.documents.is_empty() {
                {
                    let mut open_hovered = use_signal(|| false);
                    let open_bg = if open_hovered() {
                        COLOR_ACCENT_PRIMARY_HOVER
                    } else {
                        COLOR_ACCENT_PRIMARY
                    };
                    rsx! {
                        button {
                            style: format!(
                                "background: {bg}; color: {fg}; \
                                 border: none; border-radius: {r}px; \
                                 min-height: {touch}px; width: 100%; \
                                 font-size: {size}px; font-weight: {weight}; \
                                 cursor: pointer; margin-top: {mt}px;",
                                bg     = open_bg,
                                fg     = COLOR_TEXT_ON_CHROME,
                                r      = RADIUS_SM,
                                touch  = TOUCH_MIN,
                                size   = FONT_SIZE_BODY,
                                weight = FONT_WEIGHT_SEMIBOLD,
                                mt     = SPACE_2,
                            ),
                            onmouseenter: move |_| { open_hovered.set(true); },
                            onmouseleave: move |_| { open_hovered.set(false); },
                            onclick: move |_| { props.on_open_file.call(()); },
                            "{props.open_file_label}"
                        }
                    }
                }
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub(crate) struct AtRecentFileListProps {
    pub documents: Vec<RecentDocument>,
    pub recent_label: String,
    pub empty_label: String,
    pub open_file_label: String,
    /// Accessible label for the ⋮ button on each document row.
    pub menu_aria_label: String,
    /// Label for the "Remove from recents" menu action.
    pub remove_label: String,
    /// Label for the "Delete file" menu action.
    pub delete_label: String,
    /// Label for the "Open as copy" menu action.
    pub open_copy_label: String,
    pub on_select: EventHandler<usize>,
    pub on_open_file: EventHandler<()>,
    /// Called with the entry index when "Remove from recents" is chosen.
    pub on_remove: EventHandler<usize>,
    /// Called with the entry index when "Delete file" is chosen.
    pub on_delete: EventHandler<usize>,
    /// Called with the entry index when "Open as copy" is chosen.
    pub on_open_copy: EventHandler<usize>,
}
