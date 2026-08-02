// SPDX-License-Identifier: Apache-2.0

//! `AtStatusBar` — bottom status bar shell component.
//!
//! Migrated and generalised from `loki-text/src/components/toolbar.rs::BottomToolbar`.
//! All previously hardcoded strings have been replaced with props.

use dioxus::prelude::*;

use crate::components::zoom_control::AtZoomControl;
use crate::responsive::use_breakpoint;
use crate::theme::use_theme;
use crate::tokens::layout::STATUS_BAR_HEIGHT;
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_2, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_XS, FONT_WEIGHT_MEDIUM};

#[path = "status_bar_chips.rs"]
mod chips;

// ── AtStatusBar ───────────────────────────────────────────────────────────────

/// Bottom application status bar.
///
/// Displays document statistics on the left and navigation controls (zoom,
/// language, collaborator count) on the right.
///
/// **Touch targets (WCAG 2.5.8):** every interactive control (notice chip,
/// view-mode toggle, zoom badge) is at least [`TOUCH_MIN`] (44 px) wide and
/// fills the bar's full [`STATUS_BAR_HEIGHT`] (24 px) — the WCAG 2.5.8 AA
/// 24 px minimum. The bar itself caps the vertical target below the suite's
/// 44 px convention; meeting it fully needs a taller bar on touch platforms
/// (a design decision tracked in the deferred-features plan, 4c.2 tail).
/// The visual chip is a smaller nested `span`; the transparent button around
/// it is the hit area.
#[component]
pub fn AtStatusBar(props: AtStatusBarProps) -> Element {
    let palette = use_theme().palette();
    let mut view_hovered = use_signal(|| false);
    let view_bg = if view_hovered() {
        "#444444"
    } else {
        palette.surface_3
    };
    let notice_hovered = use_signal(|| false);
    let notice_bg = if notice_hovered() {
        "#444444"
    } else {
        palette.surface_3
    };
    let show_view_toggle = !props.view_mode_label.is_empty();
    let show_notice = !props.notice_label.is_empty();
    // Compact (phone-width): drop the secondary stats (word count, language)
    // so the essential page / notice / view-mode / zoom items don't crowd.
    let compact = use_breakpoint().is_compact();

    rsx! {
        div {
            role: "status",
            "aria-live": "polite",
            style: format!(
                "height: {h}px; min-height: {h}px; background: {bg}; \
                 border-top: 1px solid {border}; \
                 display: flex; align-items: center; \
                 padding: 0 {pad}px; flex-shrink: 0; gap: {gap}px; \
                 font-family: {font};",
                h      = STATUS_BAR_HEIGHT,
                bg     = palette.surface_chrome,
                border = palette.border_chrome,
                pad    = SPACE_4,
                gap    = SPACE_4,
                font   = FONT_FAMILY_UI,
            ),

            // ── Left: document statistics ─────────────────────────────────────

            // Page label (e.g. "Page 1 of 4"). Hidden when empty (e.g. the
            // reflow view, which has no fixed pages).
            if !props.page_label.is_empty() {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = palette.text_on_chrome_secondary,
                    ),
                    "{props.page_label}"
                }
            }

            // Word count label (e.g. "1,847 words"). Hidden when empty or at
            // Compact width (secondary stat).
            if !props.word_count_label.is_empty() && !compact {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = palette.text_on_chrome_secondary,
                    ),
                    "{props.word_count_label}"
                }
            }

            // Optional status notice chip (e.g. recover a dismissed warning) and
            // the transient status chip ("Document saved") — split into
            // `status_bar_chips.rs` for the 300-line ceiling.
            if show_notice {
                {chips::notice_chip(
                    props.notice_label.clone(),
                    props.notice_aria_label.clone(),
                    notice_bg,
                    &palette,
                    hit_area_style(),
                    notice_hovered,
                    props.on_notice_click,
                )}
            }
            if !props.status_note_label.is_empty() {
                {chips::status_note_chip(
                    props.status_note_label.clone(),
                    &palette,
                    hit_area_style(),
                    props.on_status_note_click,
                )}
            }

            // Flex spacer — pushes right-side content to the far right
            div { style: "flex: 1;" }

            // ── Right: language, zoom, collaborators ──────────────────────────

            // Language label (e.g. "English (US)"). Hidden at Compact width.
            if !compact {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = palette.text_on_chrome_secondary,
                    ),
                    "{props.language_label}"
                }
            }

            // View-mode toggle (paginated ⇆ reflowed). Hidden unless a label is
            // supplied, so apps that do not offer the toggle are unaffected.
            // Hit area: full bar height × ≥ TOUCH_MIN wide (see component doc).
            if show_view_toggle {
                button {
                    "aria-label": props.view_mode_aria_label.clone(),
                    style: format!(
                        "{hit} background: transparent; border: none; cursor: pointer;",
                        hit = hit_area_style(),
                    ),
                    onmouseenter: move |_| { view_hovered.set(true); },
                    onmouseleave: move |_| { view_hovered.set(false); },
                    onclick: move |_| { props.on_view_mode_click.call(()); },
                    span {
                        style: chip_style(view_bg, palette.text_on_chrome_secondary),
                        "{props.view_mode_label}"
                    }
                }
            }

            // The zoom control (Spec 08 T5.4): out / readout / in, with the
            // preset menu on the readout. It replaced a single badge that cycled
            // through six values, which stopped being viable when the increment
            // sequence had to stop wrapping — see `components::zoom`.
            AtZoomControl {
                percent: props.zoom_percent,
                capability_limit_permille: props.zoom_capability_limit_permille,
                commands: props.zoom_commands,
                labels: props.zoom_labels.clone(),
                on_change: props.on_zoom_change,
                on_fit_width: props.on_zoom_fit_width,
                on_fit_page: props.on_zoom_fit_page,
                on_actual_size: props.on_zoom_actual_size,
            }

            // Collaborator badge (hidden when count is 0)
            if props.collaborator_count > 0 {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg};",
                        size = FONT_SIZE_XS,
                        fg   = palette.text_accent,
                    ),
                    "{props.collaborator_label}"
                }
            }
        }
    }
}

/// Shared hit-area style for the status bar's interactive controls: at least
/// [`TOUCH_MIN`] wide and the bar's full height, centring the visual chip.
fn hit_area_style() -> String {
    format!(
        "min-width: {w}px; height: 100%; box-sizing: border-box; padding: 0; \
         display: flex; align-items: center; justify-content: center; flex-shrink: 0;",
        w = TOUCH_MIN,
    )
}

/// Visual chip style for the view-mode / zoom badges (`bg` swaps on hover;
/// `fg` comes from the active theme palette).
fn chip_style(bg: &str, fg: &str) -> String {
    format!(
        "background: {bg}; border-radius: {r}px; color: {fg}; \
         font-size: {size}px; font-weight: {weight}; padding: {pv}px {ph}px;",
        r = RADIUS_SM,
        size = FONT_SIZE_XS,
        weight = FONT_WEIGHT_MEDIUM,
        pv = SPACE_1,
        ph = SPACE_2,
    )
}

// ── Props ─────────────────────────────────────────────────────────────────────

/// Props for [`AtStatusBar`].
#[path = "status_bar_props.rs"]
mod props;
pub use props::AtStatusBarProps;
