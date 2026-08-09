// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The status overflow's **trigger** — the "…" button and the popover lifetime
//! it owns. Split from [`super`] for the 300-line ceiling: that file states
//! *what* the menu is (its rows, size and placement), this one *when* it is on
//! screen.

use dioxus::prelude::*;

use super::{status_overflow_request, StatusOverflowRow, STATUS_OVERFLOW_POPOVER_ID};
use crate::components::popover::{use_popover_anchor, Rect};
use crate::tokens;

/// The "More" trigger plus its hosted menu, rendered only when the fit engine
/// dropped something.
///
/// # Touch target
///
/// [`tokens::TOUCH_MIN`] wide and the bar's full height — the same hit-area
/// posture (and the same 24 px vertical shortfall) as the bar's other controls,
/// documented on [`super::status_bar::AtStatusBar`].
#[component]
pub(in crate::components::status_bar) fn AtStatusOverflow(
    rows: Vec<StatusOverflowRow>,
    aria_label: String,
) -> Element {
    let mut menu_open = use_signal(|| false);
    let popover = use_popover_anchor(STATUS_OVERFLOW_POPOVER_ID);
    let window = crate::use_window_size();
    let insets = crate::use_safe_area();
    let mut anchor = use_signal(|| Option::<MountedEvent>::None);
    let mut anchor_rect = use_signal(|| Option::<Rect>::None);

    // A widen that removes the overflow must not leave a stale-open menu: its
    // trigger is gone, so nothing could toggle it shut. The bar keeps this
    // component mounted only while `rows` is non-empty, but the row set can also
    // shrink to empty within one render, so reconcile rather than rely on the
    // unmount.
    if rows.is_empty() && *menu_open.peek() {
        menu_open.set(false);
    }

    {
        let rows = rows.clone();
        use_effect(move || {
            let Some(popover) = popover else { return };
            if !menu_open() {
                popover.dismiss();
                return;
            }
            let Some(rect) = *anchor_rect.read() else {
                // `onmounted` and `get_client_rect` both land after the first
                // render; opening without a rect would place the menu at the
                // origin. The effect re-runs when the rect arrives.
                return;
            };
            popover.open(
                status_overflow_request(
                    rows.clone(),
                    rect,
                    anchor.peek().as_ref().map(|e: &MountedEvent| e.data()),
                    menu_open,
                ),
                window,
                insets,
            );
        });
    }

    rsx! {
        button {
            "aria-label": aria_label,
            "aria-haspopup": "true",
            "aria-expanded": if menu_open() { "true" } else { "false" },
            style: format!(
                "min-width: {w}px; height: 100%; box-sizing: border-box; padding: 0; \
                 display: flex; align-items: center; justify-content: center; \
                 flex-shrink: 0; background: transparent; border: none; cursor: pointer; \
                 font-size: {size}px; color: {fg};",
                w = tokens::TOUCH_MIN,
                size = tokens::FONT_SIZE_XS,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            onmounted: move |evt: MountedEvent| {
                let data = evt.data();
                anchor.set(Some(evt));
                spawn(async move {
                    if let Ok(r) = data.get_client_rect().await {
                        anchor_rect.set(Some(Rect {
                            x: r.origin.x as f32,
                            y: r.origin.y as f32,
                            width: r.size.width as f32,
                            height: r.size.height as f32,
                        }));
                    }
                });
            },
            onclick: move |_| {
                let now = *menu_open.peek();
                menu_open.set(!now);
            },
            "…"
        }
    }
}
