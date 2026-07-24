// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `MacroDialogFrame` — the anti-spoof frame shared by every macro-originated
//! dialog (macro spec §5.5, threat T7).
//!
//! Macro-shown UI (permission prompts, and later `MsgBox`/`InputBox`) renders
//! inside a visually distinct frame that app chrome **never** uses: a violet
//! [`COLOR_MACRO_BADGE`] border and a "Macro: <project>" badge header carrying
//! the host document's title. A malicious macro therefore cannot paint a dialog
//! that looks like a genuine app dialog — the badge and reserved accent are the
//! tell.
//!
//! Mounting contract (same as [`super::super::confirm_dialog`]): the mounting
//! parent (or an ancestor) must be `position: relative` and span the area to
//! dim. `position: fixed` collapses to `absolute` in `stylo_taffy` and must not
//! be used.

use dioxus::prelude::*;

use crate::tokens::colors::{
    COLOR_MACRO_BADGE, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_2, SPACE_4};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_LABEL, FONT_SIZE_MD, FONT_WEIGHT_BOLD, FONT_WEIGHT_SEMIBOLD,
};

/// Width of a macro dialog card in logical pixels (narrow enough for Compact
/// phones; the backdrop centring keeps it on-screen).
const CARD_WIDTH_PX: f32 = 360.0;

/// Props for [`MacroDialogFrame`].
#[derive(Props, Clone, PartialEq)]
pub struct MacroDialogFrameProps {
    /// The word for "Macro" (i18n), shown in the badge chip.
    pub badge_label: String,
    /// The macro project's name (from the document), shown after the badge.
    pub project_name: String,
    /// The host document's title, shown as the frame's secondary identity line.
    pub document_title: String,
    /// Invoked when the backdrop is clicked (treated as a cancel/deny by the
    /// hosting dialog).
    pub on_backdrop: EventHandler<()>,
    /// The dialog body (message + action buttons).
    pub children: Element,
}

/// The badged backdrop + card. See the module docs for the anti-spoof rationale
/// and the positioned-ancestor mounting contract.
#[component]
pub fn MacroDialogFrame(props: MacroDialogFrameProps) -> Element {
    let badge = format!(
        "display: inline-flex; align-items: center; gap: {gap}px; \
         padding: {py}px {px}px; border-radius: {r}px; background: {badge_bg}; \
         color: #FFFFFF; font-size: {fs}px; font-weight: {fw}; \
         text-transform: uppercase; letter-spacing: 0.04em;",
        gap = SPACE_1,
        py = SPACE_1,
        px = SPACE_2,
        r = RADIUS_SM,
        badge_bg = COLOR_MACRO_BADGE,
        fs = FONT_SIZE_LABEL,
        fw = FONT_WEIGHT_BOLD,
    );

    rsx! {
        // Backdrop: dims + click-blocks; clicking cancels.
        div {
            style: "position: absolute; top: 0; left: 0; width: 100%; height: 100%; \
                    z-index: 2100; background: rgba(0, 0, 0, 0.55); \
                    display: flex; align-items: center; justify-content: center;",
            role: "presentation",
            onclick: move |_| props.on_backdrop.call(()),

            // The macro card — violet border marks it as macro-originated.
            div {
                style: format!(
                    "width: {w}px; max-width: 92%; box-sizing: border-box; \
                     display: flex; flex-direction: column; gap: {gap}px; \
                     background: {bg}; border: 2px solid {accent}; \
                     border-radius: {r}px; padding: {pad}px; \
                     font-family: {font}; color: {fg};",
                    w = CARD_WIDTH_PX,
                    gap = SPACE_2,
                    bg = COLOR_SURFACE_1,
                    accent = COLOR_MACRO_BADGE,
                    r = RADIUS_MD,
                    pad = SPACE_4,
                    font = FONT_FAMILY_UI,
                    fg = COLOR_TEXT_ON_CHROME,
                ),
                role: "dialog",
                "aria-label": format!("{}: {}", props.badge_label, props.project_name),
                onclick: move |evt| evt.stop_propagation(),

                // Anti-spoof header: badge + project name, then the document title.
                div {
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; gap: {gap}px;",
                        gap = SPACE_2,
                    ),
                    span { style: "{badge}", "aria-hidden": "true", "⚡ {props.badge_label}" }
                    span {
                        style: format!(
                            "font-size: {fs}px; font-weight: {fw}; color: {fg}; \
                             overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                            fs = FONT_SIZE_MD,
                            fw = FONT_WEIGHT_SEMIBOLD,
                            fg = COLOR_TEXT_ON_CHROME,
                        ),
                        {props.project_name.clone()}
                    }
                }
                div {
                    style: format!(
                        "font-size: {fs}px; color: {fg}; overflow: hidden; \
                         text-overflow: ellipsis; white-space: nowrap;",
                        fs = FONT_SIZE_LABEL,
                        fg = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    {props.document_title.clone()}
                }

                // Body supplied by the hosting dialog.
                {props.children}
            }
        }
    }
}
