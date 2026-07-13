// SPDX-License-Identifier: Apache-2.0

//! The status bar's left-cluster chips (notice + transient status), split from
//! `status_bar.rs` for the 300-line ceiling. Plain render helpers — no hooks —
//! so `AtStatusBar` owns the hover signals and calls these unconditionally.

use dioxus::prelude::*;

use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_2};
use crate::tokens::typography::{FONT_SIZE_XS, FONT_WEIGHT_MEDIUM};
use crate::tokens::ThemePalette;

/// The warning-coloured notice chip (e.g. "N fonts substituted").
///
/// Touch target: the transparent button uses the shared `hit_area` style
/// (≥ 44 px wide × full bar height); the visible chip is the nested span.
#[allow(clippy::too_many_arguments)]
pub(super) fn notice_chip(
    label: String,
    aria_label: String,
    bg: &'static str,
    palette: &ThemePalette,
    hit_area: String,
    mut hovered: Signal<bool>,
    on_click: Callback<()>,
) -> Element {
    rsx! {
        button {
            "aria-label": aria_label,
            style: format!("{hit_area} background: transparent; border: none; cursor: pointer;"),
            onmouseenter: move |_| { hovered.set(true); },
            onmouseleave: move |_| { hovered.set(false); },
            onclick: move |_| { on_click.call(()); },
            span {
                style: format!(
                    "background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
                     color: {fg}; font-size: {size}px; font-weight: {weight}; \
                     padding: {pv}px {ph}px;",
                    bg     = bg,
                    border = palette.contextual_tab,
                    r      = RADIUS_SM,
                    fg     = palette.text_on_chrome_secondary,
                    size   = FONT_SIZE_XS,
                    weight = FONT_WEIGHT_MEDIUM,
                    pv     = SPACE_1,
                    ph     = SPACE_2,
                ),
                "⚠ {label}"
            }
        }
    }
}

/// The neutral transient status chip (e.g. "Document saved"). The app owns the
/// message's lifetime (auto-clear / clear-on-edit); clicking dismisses.
///
/// Touch target: same convention as [`notice_chip`].
pub(super) fn status_note_chip(
    label: String,
    palette: &ThemePalette,
    hit_area: String,
    on_click: Callback<()>,
) -> Element {
    rsx! {
        button {
            "aria-label": label.clone(),
            style: format!("{hit_area} background: transparent; border: none; cursor: pointer;"),
            onclick: move |_| { on_click.call(()); },
            span {
                style: format!(
                    "background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
                     color: {fg}; font-size: {size}px; padding: {pv}px {ph}px;",
                    bg     = palette.surface_3,
                    border = palette.border_chrome,
                    r      = RADIUS_SM,
                    fg     = palette.text_on_chrome_secondary,
                    size   = FONT_SIZE_XS,
                    pv     = SPACE_1,
                    ph     = SPACE_2,
                ),
                "{label}"
            }
        }
    }
}
