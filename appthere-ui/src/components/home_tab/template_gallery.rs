// SPDX-License-Identifier: Apache-2.0

//! `AtTemplateGallery` — wrapping template grid with vertical scroll
//! (Spec 08 T4.4, I-03).
//!
//! # It was a horizontal scroller, and that is what I-03 is about
//!
//! `flex-direction: row` + `overflow-x: auto` + `flex-shrink: 0` — **a
//! horizontal-only scroll region**, which Phase 4's acceptance forbids outright.
//! Worse than it looks in two ways: on touch it has no affordance beyond
//! guessing that it scrolls sideways, and on a desktop it is the one axis a
//! wheel does not move. Templates past the first screenful were, in practice,
//! undiscoverable.
//!
//! It now wraps and scrolls **vertically**. The size class decides only whether
//! the height is capped — see [`super::gallery_layout`], where that decision
//! lives as a pure function so the narrow branch is testable on a machine that
//! is never narrow.
//!
//! The cards are child `#[component]`s so each owns its hook scope — the
//! hover signals used to be `use_signal` calls inside the gallery's `for`
//! loop and `if` arm, making the gallery's hook count depend on its props
//! (audit F6a / ADR-0013).

use dioxus::prelude::*;

use super::gallery_layout::{
    gallery_layout, CARD_GAP_PX, CARD_HEIGHT_PX, CARD_WIDTH_PX, SWATCH_HEIGHT_PX,
};
use crate::components::home_tab::BuiltinTemplate;
use crate::responsive::use_breakpoint;
use crate::tokens::colors::{COLOR_ACCENT_PRIMARY, COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME};
use crate::tokens::spacing::{RADIUS_LG, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};

// ── AtTemplateGallery ─────────────────────────────────────────────────────────

/// Wrapping grid of template cards, scrolling vertically.
///
/// Each card is a touch target satisfying the minimum:
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
///
/// A trailing "Browse…" card triggers `on_browse`.
#[component]
pub(crate) fn AtTemplateGallery(props: AtTemplateGalleryProps) -> Element {
    // **Measured viewport, never platform** (I-03, L08-011). An Android build
    // may be running on laptop-class hardware with a desktop shell, so the
    // question is how wide the window is and never which OS compiled it.
    let layout = gallery_layout(use_breakpoint());
    let cap = match layout.max_height_px {
        Some(px) => format!("max-height: {px}px; overflow-y: auto;"),
        // No `overflow-y` at all when uncapped: `auto` on an unbounded box is
        // inert, but stating it would suggest a scroll region that cannot exist.
        None => String::new(),
    };
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; flex-wrap: wrap; \
                 align-content: flex-start; gap: {gap}px; {cap} \
                 padding-bottom: {pb}px; ",
                // COMPAT(dioxus-native): flex-wrap and overflow-y: auto are both
                // in the confirmed set (CLAUDE.md); `align-content: flex-start`
                // is not — without it a capped grid with one row would centre
                // that row vertically in the cap. Verify at runtime.
                gap  = CARD_GAP_PX,
                cap  = cap,
                pb   = SPACE_2,
            ),

            for (idx, tmpl) in props.templates.iter().enumerate() {
                TemplateCard {
                    key: "{tmpl.name}",
                    idx,
                    name: tmpl.name.clone(),
                    format_label: tmpl.format_label.clone(),
                    on_select: props.on_select,
                }
            }

            // Browse… card — hidden when browse_label is empty.
            if !props.browse_label.is_empty() {
                BrowseCard {
                    label: props.browse_label.clone(),
                    on_browse: props.on_browse,
                }
            }
        }
    }
}

/// The hover border shared by both card kinds (`2px` accent when hovered).
fn hover_border(hovered: bool) -> String {
    if hovered {
        format!("border: 2px solid {COLOR_ACCENT_PRIMARY};")
    } else {
        String::new()
    }
}

// ── TemplateCard ──────────────────────────────────────────────────────────────

/// One template card (hover state owned here, not by the gallery).
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8)** via
/// `min-height` on a 100 px-wide card.
#[component]
fn TemplateCard(
    idx: usize,
    name: String,
    format_label: String,
    on_select: EventHandler<usize>,
) -> Element {
    let mut hovered = use_signal(|| false);
    rsx! {
        button {
            "aria-label": name.clone(),
            style: format!(
                "flex: 0 0 auto; width: {w}px; min-height: {touch}px; \
                 background: {bg}; border-radius: {r}px; \
                 padding: {pad}px; border: none; cursor: pointer; \
                 display: flex; flex-direction: column; \
                 align-items: center; gap: {gap}px; \
                 box-sizing: border-box; {border}",
                w      = CARD_WIDTH_PX,
                touch  = TOUCH_MIN,
                bg     = COLOR_SURFACE_PAGE,
                r      = RADIUS_LG,
                pad    = SPACE_3,
                gap    = SPACE_2,
                border = hover_border(hovered()),
            ),
            onmouseenter: move |_| { hovered.set(true); },
            onmouseleave: move |_| { hovered.set(false); },
            onclick: move |_| { on_select.call(idx); },

            // Format swatch placeholder
            // TODO(icons): Replace with format-type illustration.
            div {
                style: format!(
                    "width: 60px; height: {h}px; \
                     background: #DDDDDD; border-radius: {r}px; \
                     display: flex; align-items: flex-end; \
                     justify-content: center; padding-bottom: {p}px;",
                    h = SWATCH_HEIGHT_PX,
                    r = RADIUS_SM,
                    p = SPACE_1,
                ),
                span {
                    style: format!(
                        "font-size: {size}px; color: #888888;",
                        size = FONT_SIZE_LABEL,
                    ),
                    "{format_label}"
                }
            }
            span {
                style: format!(
                    "font-size: {size}px; font-weight: {weight}; \
                     color: #1A1A1A; text-align: center;",
                    size   = FONT_SIZE_LABEL,
                    weight = FONT_WEIGHT_SEMIBOLD,
                ),
                "{name}"
            }
        }
    }
}

// ── BrowseCard ────────────────────────────────────────────────────────────────

/// The trailing "Browse…" card (hover state owned here, not by the gallery).
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8)** via
/// `min-height` on a 100 px-wide card.
#[component]
fn BrowseCard(label: String, on_browse: EventHandler<()>) -> Element {
    let mut hovered = use_signal(|| false);
    rsx! {
        button {
            "aria-label": label.clone(),
            style: format!(
                "flex: 0 0 auto; width: {w}px; min-height: {h}px; \
                 background: transparent; border-radius: {r}px; \
                 padding: {pad}px; cursor: pointer; \
                 display: flex; flex-direction: column; \
                 align-items: center; justify-content: center; \
                 gap: {gap}px; box-sizing: border-box; {border}",
                w      = CARD_WIDTH_PX,
                // Matches a template card so the Browse tile does not make the
                // last row taller than the cap was derived for.
                h      = CARD_HEIGHT_PX,
                r      = RADIUS_LG,
                pad    = SPACE_3,
                gap    = SPACE_2,
                border = hover_border(hovered()),
            ),
            onmouseenter: move |_| { hovered.set(true); },
            onmouseleave: move |_| { hovered.set(false); },
            onclick: move |_| { on_browse.call(()); },
            span {
                style: format!(
                    "font-size: {size}px; font-weight: {weight}; color: {fg};",
                    size   = FONT_SIZE_BODY,
                    weight = FONT_WEIGHT_SEMIBOLD,
                    fg     = COLOR_TEXT_ON_CHROME,
                ),
                "{label}"
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub(crate) struct AtTemplateGalleryProps {
    pub templates: Vec<BuiltinTemplate>,
    pub browse_label: String,
    pub on_select: EventHandler<usize>,
    pub on_browse: EventHandler<()>,
}
