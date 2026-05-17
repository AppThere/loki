// SPDX-License-Identifier: Apache-2.0

//! `AtRecentFileList` — vertically scrollable recent documents list.

use dioxus::prelude::*;

use crate::components::home_tab::RecentDocument;
use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_ACCENT_PRIMARY_HOVER, COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD,
};

// ── AtRecentFileList ──────────────────────────────────────────────────────────

/// Vertically scrollable list of recently opened documents.
///
/// Shows an empty state with an "Open File" button when `documents` is empty.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
/// Each document row meets this requirement.
#[component]
pub(crate) fn AtRecentFileList(props: AtRecentFileListProps) -> Element {
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
                                 align-items: center; gap: {gap}px; \
                                 padding: {p}px;",
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

            for (idx, doc) in props.documents.iter().enumerate() {
                {
                    let title = doc.title.clone();
                    let path = doc.path.clone();
                    let modified = doc.modified_at.clone();
                    let mut row_hovered = use_signal(|| false);
                    let row_bg = if row_hovered() { "#F5F5F5" } else { COLOR_SURFACE_PAGE };
                    rsx! {
                        button {
                            key: "{idx}",
                            "aria-label": title.clone(),
                            style: format!(
                                "background: {bg}; border: none; border-radius: {r}px; \
                                 padding: {pv}px {ph}px; \
                                 min-height: {touch}px; width: 100%; \
                                 display: flex; flex-direction: column; gap: {gap}px; \
                                 cursor: pointer; text-align: left; box-sizing: border-box;",
                                bg    = row_bg,
                                r     = RADIUS_MD,
                                pv    = SPACE_3,
                                ph    = SPACE_4,
                                touch = TOUCH_MIN,
                                gap   = SPACE_1,
                            ),
                            onmouseenter: move |_| { row_hovered.set(true); },
                            onmouseleave: move |_| { row_hovered.set(false); },
                            onclick: move |_| { props.on_select.call(idx); },

                            span {
                                style: format!(
                                    "font-size: {size}px; font-weight: {weight}; \
                                     color: {fg};",
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
    pub on_select: EventHandler<usize>,
    pub on_open_file: EventHandler<()>,
}
