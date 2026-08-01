// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! One row of [`super::recent_files::AtRecentFileList`]: the document info
//! button and the ⋮ menu toggle.
//!
//! # The menu moved out (Spec 08 T4.2)
//!
//! It used to render **here**, inside the row, expanding it and pushing every
//! row below it down. It is now a popover hosted at the app root — see
//! `super::recent_menu` for what that fixes. What is left is the trigger, and a
//! trigger's job for an anchored overlay is to report *where it is*: only the
//! button knows its own rect.
//!
//! A `#[component]` (not a loop body) so it owns its hook scope — the hover
//! signal used to be a `use_signal` inside the list's `for` loop, which made
//! the parent's hook count depend on the document count (audit F6a /
//! ADR-0013).

use dioxus::prelude::*;

use crate::components::popover::Rect;

use crate::tokens::colors::{
    COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME_SECONDARY, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_3, SPACE_4, TOUCH_MIN};
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
    pub on_select: EventHandler<usize>,
    /// Toggle, carrying the button's **window** rect so the parent can anchor a
    /// popover to it. `None` when the rect could not be read.
    pub on_toggle_menu: EventHandler<(usize, Option<Rect>)>,
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
    // The button's `MountedData`, captured at mount so the click handler can read
    // its rect. Read at click time rather than at mount: the rect changes with
    // every scroll of the list, so the moment of the click is the only moment it
    // is the answer.
    let mut trigger = use_signal(|| Option::<MountedEvent>::None);

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
                    onmounted: move |evt: MountedEvent| { trigger.set(Some(evt)); },
                    onclick: move |_| {
                        let on_toggle = props.on_toggle_menu;
                        let Some(evt) = trigger.peek().clone() else {
                            // No mounted handle: report the toggle with no rect
                            // rather than swallowing the click. The parent
                            // closes on `None`, so the button stays a toggle —
                            // a trigger that silently does nothing is the dead
                            // control this primitive exists to avoid.
                            on_toggle.call((idx, None));
                            return;
                        };
                        spawn(async move {
                            // Async because `get_client_rect` round-trips to the
                            // event loop; the rect is read here rather than kept
                            // fresh because keeping it fresh would mean a second
                            // scroll subscription per row.
                            let rect = evt.get_client_rect().await.ok().map(|r| Rect {
                                x: r.origin.x as f32,
                                y: r.origin.y as f32,
                                width: r.size.width as f32,
                                height: r.size.height as f32,
                            });
                            on_toggle.call((idx, rect));
                        });
                    },
                    "⋮"
                }
            }

        }
    }
}
