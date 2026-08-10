// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtDialogTabStrip`] — the responsive tab strip of a tabbed dialog.
//!
//! Presentation per size class comes from [`super::DialogTabLayout`]; this file
//! only paints it. All three presentations expose the **same tab set** — a tab
//! is never dropped, only moved between the inline row, the `More ▾` menu, and
//! the Compact picker (design notes 03/04).
//!
//! # The menu and the picker are disclosure rows, not popovers
//!
//! Both render **in flow, directly under the strip**, rather than as anchored
//! popovers. A popover here would be anchored to an element inside an inline
//! formatting context, where `position: absolute` is still unverified in Blitz
//! (root `CLAUDE.md`), and an overflow menu that exceeds the viewport is the
//! worst small-screen failure mode there is (design note 10). The in-flow list
//! costs vertical space and cannot fail that way.
//!
//! # Touch targets
//!
//! Inline tabs and the `More ▾` trigger are [`tokens::TOUCH_MIN`] tall at
//! Compact and desktop-density above it, matching `AtRibbonTabStrip`. Rows in
//! the open menu and the Compact picker are always [`tokens::TOUCH_MIN`] tall —
//! they are the only way to reach a hidden tab, so they are touch-sized at every
//! size class (WCAG 2.5.8).

use dioxus::prelude::*;

use super::tabs::DialogTabLayout;
use crate::responsive::{use_breakpoint, Breakpoint};
use crate::tokens;

/// Renders the strip for `labels`, highlighting `active`.
///
/// `menu_open` is owned by the caller so the dialog can close the menu when the
/// user picks a tab, and so the state survives a body redraw.
#[component]
pub fn AtDialogTabStrip(
    /// Localized tab labels, in strip order.
    labels: Vec<String>,
    /// Index of the active tab.
    active: usize,
    /// How many tabs keep an inline slot at the Medium size class.
    #[props(default = tokens::DIALOG_TABS_INLINE_MEDIUM)]
    inline_at_medium: usize,
    /// Whether the overflow menu / Compact picker list is expanded.
    menu_open: Signal<bool>,
    /// Localized label for the overflow trigger (e.g. `More`).
    more_label: String,
    /// Fired with the index of the newly selected tab.
    on_select: EventHandler<usize>,
) -> Element {
    let bp = use_breakpoint();
    let total = labels.len();
    let layout = DialogTabLayout::for_breakpoint(bp, total, inline_at_medium);
    let inline = layout.inline_indices(total, active);
    let hidden = layout.overflow_indices(total, active);
    let strip_h = if bp == Breakpoint::Compact {
        tokens::TOUCH_MIN
    } else {
        tokens::RIBBON_TAB_STRIP_HEIGHT
    };
    let is_picker = layout == DialogTabLayout::Picker;
    let open = *menu_open.read();

    rsx! {
        div {
            style: format!(
                "flex-shrink: 0; display: flex; flex-direction: column; \
                 border-bottom: 1px solid {border};",
                border = tokens::COLOR_BORDER_CHROME,
            ),

            if is_picker {
                // ── Compact: one picker row standing in for the whole strip ───
                div {
                    style: format!("padding: {p}px {px}px;", p = tokens::SPACE_3, px = tokens::SPACE_4),
                    button {
                        role: "combobox",
                        aria_expanded: if open { "true" } else { "false" },
                        style: picker_trigger_style(),
                        onclick: move |evt| {
                            evt.stop_propagation();
                            let now = *menu_open.read();
                            menu_open.set(!now);
                        },
                        span {
                            style: format!(
                                "display: flex; flex-direction: row; align-items: center; gap: {gap}px;",
                                gap = tokens::SPACE_2,
                            ),
                            span {
                                style: format!(
                                    "font-size: {fs}px; color: {fg};",
                                    fs = tokens::FONT_SIZE_LABEL,
                                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                ),
                                // Position in the set — the affordance that
                                // tells the user how much they have not seen.
                                {format!("{} / {}", active.saturating_add(1).min(total.max(1)), total)}
                            }
                            span {
                                style: format!("color: {};", tokens::COLOR_TEXT_ON_CHROME),
                                {labels.get(active).cloned().unwrap_or_default()}
                            }
                        }
                        span { style: format!("color: {};", tokens::COLOR_TEXT_ON_CHROME_SECONDARY), "\u{25BE}" }
                    }
                }
            } else {
                // ── Expanded / Medium: inline labels (+ More ▾ when collapsed) ─
                div {
                    role: "tablist",
                    style: format!(
                        "display: flex; flex-direction: row; align-items: stretch; \
                         height: {h}px; padding: 0 {px}px;",
                        h = strip_h,
                        px = tokens::SPACE_3,
                    ),
                    for idx in inline {
                        button {
                            key: "{idx}",
                            role: "tab",
                            aria_selected: if idx == active { "true" } else { "false" },
                            style: inline_tab_style(idx == active),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                menu_open.set(false);
                                on_select.call(idx);
                            },
                            {labels.get(idx).cloned().unwrap_or_default()}
                        }
                    }
                    if !hidden.is_empty() {
                        // Spacer pushes the trigger to the trailing edge.
                        div { style: "flex: 1;" }
                        button {
                            aria_expanded: if open { "true" } else { "false" },
                            style: more_trigger_style(),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                let now = *menu_open.read();
                                menu_open.set(!now);
                            },
                            {more_label}
                            span { style: format!("color: {};", tokens::COLOR_TEXT_ON_CHROME_SECONDARY), "\u{25BE}" }
                            span {
                                style: format!(
                                    "font-size: {fs}px; color: {fg};",
                                    fs = tokens::FONT_SIZE_XS,
                                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                ),
                                {hidden.len().to_string()}
                            }
                        }
                    }
                }
            }

            // ── The disclosure list, shared by the menu and the picker ────────
            if open && !hidden.is_empty() {
                div {
                    role: "listbox",
                    style: format!(
                        "display: flex; flex-direction: column; \
                         border-top: 1px solid {border}; background: {bg};",
                        border = tokens::COLOR_BORDER_CHROME,
                        bg = tokens::COLOR_SURFACE_2,
                    ),
                    for idx in hidden {
                        button {
                            key: "menu-{idx}",
                            role: "option",
                            aria_selected: if idx == active { "true" } else { "false" },
                            style: menu_row_style(idx == active),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                menu_open.set(false);
                                on_select.call(idx);
                            },
                            {labels.get(idx).cloned().unwrap_or_default()}
                        }
                    }
                }
            }
        }
    }
}

/// An inline tab label; the active one carries the 2 px accent underline.
fn inline_tab_style(active: bool) -> String {
    let underline = if active {
        format!(
            "border-bottom: 2px solid {};",
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        )
    } else {
        String::new()
    };
    format!(
        "padding: 0 {px}px; display: flex; align-items: center; \
         background: transparent; border: none; cursor: pointer; \
         white-space: nowrap; box-sizing: border-box; \
         font-size: {fs}px; color: {fg}; {underline}",
        px = tokens::SPACE_3,
        fs = tokens::FONT_SIZE_BODY,
        fg = if active {
            tokens::COLOR_TEXT_ON_CHROME
        } else {
            tokens::COLOR_TEXT_ON_CHROME_SECONDARY
        },
    )
}

/// The `More ▾ <n>` trigger.
fn more_trigger_style() -> String {
    format!(
        "align-self: center; display: flex; flex-direction: row; \
         align-items: center; gap: {gap}px; padding: {py}px {px}px; \
         background: transparent; border: 1px solid {border}; \
         border-radius: {r}px; cursor: pointer; white-space: nowrap; \
         font-size: {fs}px; color: {fg};",
        gap = tokens::SPACE_1,
        py = tokens::SPACE_1,
        px = tokens::SPACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_MD,
        fs = tokens::FONT_SIZE_BODY,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}

/// The Compact picker trigger — a full-width touch-sized combobox row.
fn picker_trigger_style() -> String {
    format!(
        "width: 100%; min-height: {t}px; box-sizing: border-box; \
         display: flex; flex-direction: row; align-items: center; \
         justify-content: space-between; padding: 0 {px}px; \
         background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
         cursor: pointer; font-size: {fs}px; color: {fg};",
        t = tokens::TOUCH_MIN,
        px = tokens::SPACE_3,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_MD,
        fs = tokens::FONT_SIZE_MD,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}

/// A row in the open disclosure list — touch-sized at every size class, since
/// it is the only route to a hidden tab.
fn menu_row_style(active: bool) -> String {
    format!(
        "min-height: {t}px; box-sizing: border-box; display: flex; \
         align-items: center; padding: 0 {px}px; text-align: left; \
         background: {bg}; border: none; \
         border-bottom: 1px solid {border}; cursor: pointer; \
         font-size: {fs}px; color: {fg};",
        t = tokens::TOUCH_MIN,
        px = tokens::SPACE_4,
        bg = if active {
            tokens::COLOR_SURFACE_3
        } else {
            "transparent"
        },
        border = tokens::COLOR_BORDER_CHROME,
        fs = tokens::FONT_SIZE_MD,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}
