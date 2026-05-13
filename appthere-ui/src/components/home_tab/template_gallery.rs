// SPDX-License-Identifier: Apache-2.0

//! `AtTemplateGallery` — horizontally scrollable template card row.

use dioxus::prelude::*;

use crate::components::home_tab::BuiltinTemplate;
use crate::tokens::colors::{COLOR_ACCENT_PRIMARY, COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME};
use crate::tokens::spacing::{RADIUS_LG, RADIUS_SM, SPACE_2, SPACE_3, TOUCH_MIN};
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
                {
                    let name = tmpl.name;
                    let fmt_label = tmpl.format_label;
                    let mut hovered = use_signal(|| false);
                    let border = if hovered() {
                        format!("border: 2px solid {COLOR_ACCENT_PRIMARY};")
                    } else {
                        String::new()
                    };
                    rsx! {
                        button {
                            key: "{idx}",
                            "aria-label": name,
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
                                border = border,
                            ),
                            onmouseenter: move |_| { hovered.set(true); },
                            onmouseleave: move |_| { hovered.set(false); },
                            onclick: move |_| { props.on_select.call(idx); },

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
                                    "{fmt_label}"
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
            }

            // Browse… card — hidden when browse_label is empty.
            if !props.browse_label.is_empty() {
                {
                    let mut browse_hovered = use_signal(|| false);
                    let browse_border = if browse_hovered() {
                        format!("border: 2px solid {COLOR_ACCENT_PRIMARY};")
                    } else {
                        String::new()
                    };
                    rsx! {
                        button {
                            "aria-label": props.browse_label,
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
                                border = browse_border,
                            ),
                            onmouseenter: move |_| { browse_hovered.set(true); },
                            onmouseleave: move |_| { browse_hovered.set(false); },
                            onclick: move |_| { props.on_browse.call(()); },
                            span {
                                style: format!(
                                    "font-size: {size}px; font-weight: {weight}; color: {fg};",
                                    size   = FONT_SIZE_BODY,
                                    weight = FONT_WEIGHT_SEMIBOLD,
                                    fg     = COLOR_ON_CHROME_BROWSE,
                                ),
                                "{props.browse_label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

// Fallback color constant for the browse card text (not in the main palette yet)
const COLOR_ON_CHROME_BROWSE: &str = COLOR_TEXT_ON_CHROME;

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub(crate) struct AtTemplateGalleryProps {
    pub templates: Vec<BuiltinTemplate>,
    pub browse_label: &'static str,
    pub on_select: EventHandler<usize>,
    pub on_browse: EventHandler<()>,
}

// Keep lint happy — the SPACE_1 constant is used in the swatch sub-element.
use crate::tokens::spacing::SPACE_1;
