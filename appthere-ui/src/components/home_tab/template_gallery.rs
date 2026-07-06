// SPDX-License-Identifier: Apache-2.0

//! `AtTemplateGallery` — horizontally scrollable template card row.
//!
//! The cards are child `#[component]`s so each owns its hook scope — the
//! hover signals used to be `use_signal` calls inside the gallery's `for`
//! loop and `if` arm, making the gallery's hook count depend on its props
//! (audit F6a / ADR-0013).

use dioxus::prelude::*;

use crate::components::home_tab::BuiltinTemplate;
use crate::tokens::colors::{COLOR_ACCENT_PRIMARY, COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME};
use crate::tokens::spacing::{RADIUS_LG, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD,
};

// ── AtTemplateGallery ─────────────────────────────────────────────────────────

/// Horizontally scrollable row of template cards.
///
/// Each card is a touch target satisfying the minimum:
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
///
/// A trailing "Browse…" card triggers `on_browse`.
#[component]
pub(crate) fn AtTemplateGallery(props: AtTemplateGalleryProps) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; gap: {gap}px; \
                 overflow-x: auto; padding-bottom: {pb}px; \
                 font-family: {font};",
                // COMPAT(dioxus-native): overflow-x: auto is confirmed working.
                // scrollbar-width: none is unconfirmed — verify at runtime.
                gap  = SPACE_3,
                pb   = SPACE_2,
                font = FONT_FAMILY_UI,
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
                "flex-shrink: 0; width: 100px; min-height: {touch}px; \
                 background: {bg}; border-radius: {r}px; \
                 padding: {pad}px; border: none; cursor: pointer; \
                 display: flex; flex-direction: column; \
                 align-items: center; gap: {gap}px; \
                 box-sizing: border-box; {border}",
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
                    "width: 60px; height: 72px; \
                     background: #DDDDDD; border-radius: {r}px; \
                     display: flex; align-items: flex-end; \
                     justify-content: center; padding-bottom: {p}px;",
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
                "flex-shrink: 0; width: 100px; min-height: {touch}px; \
                 background: transparent; border-radius: {r}px; \
                 padding: {pad}px; cursor: pointer; \
                 display: flex; flex-direction: column; \
                 align-items: center; justify-content: center; \
                 gap: {gap}px; box-sizing: border-box; {border}",
                touch  = TOUCH_MIN,
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
