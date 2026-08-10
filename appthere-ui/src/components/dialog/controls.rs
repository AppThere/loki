// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The two selection controls a Loki dialog is built from: the segmented
//! selector and the checkbox row. The footer buttons live in
//! [`super::button`].
//!
//! # Touch targets
//!
//! Both take `min_touch_px` from [`super::DialogPosture::min_touch_px`] and
//! reach 44×44 logical px at the Compact size class (WCAG 2.5.8).

use dioxus::prelude::*;

use crate::tokens;

/// A row of mutually-exclusive options rendered as adjacent buttons — the
/// design's Regular/Bold/Italic, Left/Centre/Right, Portrait/Landscape control.
///
/// Preferred over a dropdown whenever the option set is small and fixed: it
/// shows every choice and the current one at a glance, with no popover to
/// position (Blitz has no native `<select>`).
#[component]
pub fn AtSegmented(
    /// Localized option labels, in display order.
    options: Vec<String>,
    /// Index of the selected option.
    selected: usize,
    /// Per-option style fragments (e.g. `font-weight: 700;` to preview a
    /// weight). Shorter than `options` is fine — missing entries add nothing.
    #[props(default)]
    option_styles: Vec<String>,
    /// Minimum touch height (CSS px); 0 = pointer density.
    #[props(default = 0.0)]
    min_touch_px: f32,
    /// Render every option greyed and swallow clicks.
    #[props(default = false)]
    disabled: bool,
    /// Fired with the index of the chosen option.
    on_select: EventHandler<usize>,
) -> Element {
    let touch = if min_touch_px > 0.0 {
        format!("min-height: {min_touch_px}px;")
    } else {
        String::new()
    };
    rsx! {
        div {
            role: "radiogroup",
            style: format!(
                "display: flex; flex-direction: row; gap: {gap}px;",
                gap = tokens::SPACE_2,
            ),
            for (idx, label) in options.iter().enumerate() {
                button {
                    key: "{idx}",
                    role: "radio",
                    aria_checked: if idx == selected { "true" } else { "false" },
                    disabled,
                    style: format!(
                        "flex: 1; {touch} box-sizing: border-box; \
                         display: flex; align-items: center; justify-content: center; \
                         padding: {py}px {px}px; border-radius: {r}px; \
                         cursor: {cursor}; white-space: nowrap; font-size: {fs}px; \
                         background: {bg}; border: 1px solid {border}; color: {fg}; {opt}",
                        py = tokens::SPACE_2,
                        px = tokens::SPACE_2,
                        r = tokens::RADIUS_MD,
                        cursor = if disabled { "default" } else { "pointer" },
                        fs = tokens::FONT_SIZE_BODY,
                        bg = if idx == selected {
                            tokens::COLOR_SURFACE_3
                        } else {
                            tokens::COLOR_SURFACE_2
                        },
                        border = if idx == selected {
                            tokens::COLOR_TAB_ACTIVE_INDICATOR
                        } else {
                            tokens::COLOR_BORDER_CHROME
                        },
                        fg = if disabled {
                            tokens::COLOR_ICON_DISABLED
                        } else if idx == selected {
                            tokens::COLOR_TEXT_ON_CHROME
                        } else {
                            tokens::COLOR_TEXT_ON_CHROME_SECONDARY
                        },
                        opt = option_styles.get(idx).cloned().unwrap_or_default(),
                    ),
                    onclick: move |evt| {
                        evt.stop_propagation();
                        if !disabled {
                            on_select.call(idx);
                        }
                    },
                    {label.clone()}
                }
            }
        }
    }
}

/// A labelled boolean. Renders as a checkbox at pointer density and as a
/// full-width switch row at Compact — the same state in the idiom of the input
/// device (design note 25).
#[component]
pub fn AtCheckRow(
    /// The localized label.
    label: Element,
    /// Current state.
    checked: bool,
    /// Minimum touch height (CSS px); above 0 this renders as a switch row.
    #[props(default = 0.0)]
    min_touch_px: f32,
    /// Grey the row and swallow clicks.
    #[props(default = false)]
    disabled: bool,
    /// Accessible label, when the visual label is not plain text.
    #[props(default)]
    aria_label: Option<String>,
    /// Fired with the new state.
    on_toggle: EventHandler<bool>,
) -> Element {
    let touch_mode = min_touch_px > 0.0;
    let fg = if disabled {
        tokens::COLOR_ICON_DISABLED
    } else if checked {
        tokens::COLOR_TEXT_ON_CHROME
    } else {
        tokens::COLOR_TEXT_ON_CHROME_SECONDARY
    };
    let fill = if disabled {
        tokens::COLOR_ICON_DISABLED
    } else {
        tokens::COLOR_TAB_ACTIVE_INDICATOR
    };

    rsx! {
        button {
            role: "checkbox",
            aria_checked: if checked { "true" } else { "false" },
            aria_label,
            disabled,
            style: format!(
                "{touch} width: 100%; box-sizing: border-box; display: flex; \
                 flex-direction: row; align-items: center; gap: {gap}px; \
                 justify-content: {justify}; padding: {py}px 0; \
                 background: transparent; border: none; \
                 cursor: {cursor}; text-align: left; font-size: {fs}px; color: {fg};",
                touch = if touch_mode { format!("min-height: {min_touch_px}px;") } else { String::new() },
                gap = tokens::SPACE_3,
                justify = if touch_mode { "space-between" } else { "flex-start" },
                py = tokens::SPACE_1,
                cursor = if disabled { "default" } else { "pointer" },
                fs = tokens::FONT_SIZE_BODY,
            ),
            onclick: move |evt| {
                evt.stop_propagation();
                if !disabled {
                    on_toggle.call(!checked);
                }
            },

            if touch_mode {
                // Switch row: label first, track trailing.
                div { {label} }
                span {
                    style: format!(
                        "flex-shrink: 0; width: 44px; height: 26px; border-radius: {r}px; \
                         box-sizing: border-box; padding: 2px; display: flex; \
                         align-items: center; justify-content: {pos}; \
                         background: {bg}; border: 1px solid {border};",
                        r = tokens::RADIUS_FULL,
                        pos = if checked { "flex-end" } else { "flex-start" },
                        bg = if checked { fill } else { tokens::COLOR_SURFACE_3 },
                        border = if checked { fill } else { tokens::COLOR_BORDER_CHROME },
                    ),
                    span {
                        style: format!(
                            "width: 20px; height: 20px; border-radius: {r}px; background: {knob};",
                            r = tokens::RADIUS_FULL,
                            knob = if checked {
                                tokens::COLOR_SURFACE_CHROME
                            } else {
                                tokens::COLOR_ICON_DISABLED
                            },
                        ),
                    }
                }
            } else {
                // Checkbox: box first, label after.
                span {
                    style: format!(
                        "flex-shrink: 0; width: 17px; height: 17px; border-radius: {r}px; \
                         box-sizing: border-box; display: flex; align-items: center; \
                         justify-content: center; font-size: {fs}px; \
                         background: {bg}; border: 1px solid {border}; color: {tick};",
                        r = tokens::RADIUS_SM,
                        fs = tokens::FONT_SIZE_XS,
                        bg = if checked { fill } else { "transparent" },
                        border = if checked { fill } else { tokens::COLOR_BORDER_CHROME },
                        tick = tokens::COLOR_SURFACE_CHROME,
                    ),
                    if checked { "\u{2713}" }
                }
                div { {label} }
            }
        }
    }
}
