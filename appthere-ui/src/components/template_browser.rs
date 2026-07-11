// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtTemplateBrowser` — a blocking overlay listing every available document
//! template (the Home tab's gallery shows only the first few; the "Browse…"
//! card opens this full list).
//!
//! Same mounting contract as [`AtConfirmDialog`](super::confirm_dialog): a
//! `position: absolute` backdrop, so the mounting parent (or an ancestor)
//! must be `position: relative` and span the area to dim. Mount conditionally
//! at a component boundary (ADR-0013):
//! ```rust,ignore
//! {browsing().then(|| rsx! {
//!     AtTemplateBrowser {
//!         title: fl!("home-browse-title"),
//!         cancel_label: fl!("home-browse-cancel"),
//!         entries: template_names,
//!         on_select: move |idx| { open_template(idx); browsing.set(false); },
//!         on_cancel: move |_| browsing.set(false),
//!     }
//! })}
//! ```
//!
//! Clicking the backdrop cancels. Every template row is at least 44 logical
//! pixels tall (WCAG 2.5.8). Colors resolve through the theme palette.

use dioxus::prelude::*;

use crate::theme::use_theme;
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_MD, FONT_WEIGHT_SEMIBOLD,
};

/// Card width — matches `AtConfirmDialog`; the list scrolls inside.
const BROWSER_WIDTH_PX: f32 = 360.0;

/// Maximum list height before it scrolls (keeps the card on Compact screens).
const BROWSER_LIST_MAX_PX: f32 = 320.0;

/// Props for [`AtTemplateBrowser`]. All display strings are props
/// (i18n-agnostic).
#[derive(Props, Clone, PartialEq)]
pub struct AtTemplateBrowserProps {
    /// Dialog heading (e.g. "All templates").
    pub title: String,
    /// Label of the closing button.
    pub cancel_label: String,
    /// Display name of each selectable template, in presentation order.
    pub entries: Vec<String>,
    /// Invoked with the selected entry's index.
    pub on_select: EventHandler<usize>,
    /// Invoked when the user dismisses (button or backdrop click).
    pub on_cancel: EventHandler<()>,
}

/// Template list overlay. See the module docs for the mounting contract.
///
/// Touch targets: each template row and the close button are at least
/// **44×44 logical pixels** (WCAG 2.5.8) via `min-height: TOUCH_MIN`.
#[component]
pub fn AtTemplateBrowser(props: AtTemplateBrowserProps) -> Element {
    let palette = use_theme().palette();
    rsx! {
        div {
            style: "position: absolute; top: 0; left: 0; width: 100%; height: 100%; \
                    z-index: 2000; background: rgba(0, 0, 0, 0.45); \
                    display: flex; align-items: center; justify-content: center;",
            role: "presentation",
            onclick: move |_| props.on_cancel.call(()),

            div {
                style: format!(
                    "width: {w}px; max-width: 90%; box-sizing: border-box; \
                     display: flex; flex-direction: column; gap: {gap}px; \
                     background: {bg}; border: 1px solid {border}; \
                     border-radius: {r}px; padding: {pad}px; \
                     font-family: {font}; color: {fg};",
                    w = BROWSER_WIDTH_PX,
                    gap = SPACE_3,
                    bg = palette.surface_1,
                    border = palette.border_chrome,
                    r = RADIUS_MD,
                    pad = SPACE_4,
                    font = FONT_FAMILY_UI,
                    fg = palette.text_on_chrome,
                ),
                role: "dialog",
                "aria-label": props.title.clone(),
                onclick: move |evt| evt.stop_propagation(),

                div {
                    style: format!(
                        "font-size: {fs}px; font-weight: {fw};",
                        fs = FONT_SIZE_MD,
                        fw = FONT_WEIGHT_SEMIBOLD,
                    ),
                    {props.title.clone()}
                }

                // Scrollable template list, one full-width row per entry.
                div {
                    style: format!(
                        "display: flex; flex-direction: column; gap: {gap}px; \
                         max-height: {max}px; overflow-y: auto;",
                        gap = SPACE_2,
                        max = BROWSER_LIST_MAX_PX,
                    ),
                    for (idx, name) in props.entries.iter().enumerate() {
                        button {
                            key: "{idx}",
                            style: format!(
                                "min-height: {th}px; box-sizing: border-box; \
                                 padding: {py}px {px}px; border-radius: {r}px; \
                                 background: {bg}; border: 1px solid {border}; \
                                 color: {fg}; font-family: {font}; \
                                 font-size: {fs}px; cursor: pointer; \
                                 display: flex; align-items: center; \
                                 text-align: left;",
                                th = TOUCH_MIN,
                                py = SPACE_2,
                                px = SPACE_3,
                                r = RADIUS_SM,
                                bg = palette.surface_2,
                                border = palette.border_chrome,
                                fg = palette.text_on_chrome,
                                font = FONT_FAMILY_UI,
                                fs = FONT_SIZE_BODY,
                            ),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                props.on_select.call(idx);
                            },
                            "{name}"
                        }
                    }
                }

                div {
                    style: format!(
                        "display: flex; justify-content: flex-end; gap: {gap}px;",
                        gap = SPACE_2,
                    ),
                    button {
                        style: format!(
                            "min-height: {th}px; box-sizing: border-box; \
                             padding: {py}px {px}px; border-radius: {r}px; \
                             font-family: {font}; font-size: {fs}px; \
                             font-weight: {fw}; background: transparent; \
                             border: 1px solid {border}; color: {fg}; \
                             display: flex; align-items: center; justify-content: center;",
                            th = TOUCH_MIN,
                            py = SPACE_2,
                            px = SPACE_4,
                            r = RADIUS_MD,
                            font = FONT_FAMILY_UI,
                            fs = FONT_SIZE_BODY,
                            fw = FONT_WEIGHT_SEMIBOLD,
                            border = palette.border_chrome,
                            fg = palette.text_on_chrome,
                        ),
                        onclick: move |evt| {
                            evt.stop_propagation();
                            props.on_cancel.call(());
                        },
                        {props.cancel_label.clone()}
                    }
                }
            }
        }
    }
}
