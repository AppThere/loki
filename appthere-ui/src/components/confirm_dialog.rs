// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtConfirmDialog` — a blocking confirmation overlay for destructive or
//! data-losing actions (delete a file, close a dirty tab).
//!
//! Rendered as a `position: absolute` full-area backdrop with a centred card,
//! so the **mounting parent (or an ancestor) must be `position: relative` and
//! span the area to dim** — the same confirmed-working Blitz pattern as the
//! editor's floating spelling menu (`position: fixed` collapses to `absolute`
//! in stylo_taffy and must not be used).
//!
//! Mount it conditionally at a component boundary (ADR-0013):
//! ```rust,ignore
//! {pending().then(|| rsx! {
//!     AtConfirmDialog {
//!         title: fl!("home-delete-confirm-title"),
//!         message: fl!("home-delete-confirm-message", title = name),
//!         confirm_label: fl!("home-delete-confirm-confirm"),
//!         cancel_label: fl!("home-delete-confirm-cancel"),
//!         danger: true,
//!         on_confirm: move |_| { /* do it */ pending.set(None); },
//!         on_cancel: move |_| pending.set(None),
//!     }
//! })}
//! ```
//!
//! Clicking the backdrop cancels (same as the Cancel button). Both buttons
//! meet the 44×44 logical-pixel minimum touch target (WCAG 2.5.8) via
//! `min-height: TOUCH_MIN` and horizontal padding.

use dioxus::prelude::*;

use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_BORDER_CHROME, COLOR_STATUS_ERROR_BORDER, COLOR_SURFACE_1,
    COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_MD, FONT_WEIGHT_SEMIBOLD,
};

/// Width of the dialog card in logical pixels — narrow enough for Compact
/// portrait phones (the backdrop centring keeps it on-screen either way).
const DIALOG_WIDTH_PX: f32 = 320.0;

/// Confirmation dialog props. All display strings are props (i18n-agnostic).
#[derive(Props, Clone, PartialEq)]
pub struct AtConfirmDialogProps {
    /// Short dialog heading (e.g. "Delete file").
    pub title: String,
    /// One- or two-sentence body explaining what will happen.
    pub message: String,
    /// Label of the confirming (destructive) button.
    pub confirm_label: String,
    /// Label of the cancelling (safe) button.
    pub cancel_label: String,
    /// Style the confirm button as destructive (error border/text). Defaults
    /// to `true` — this dialog exists for destructive confirmations.
    #[props(default = true)]
    pub danger: bool,
    /// Invoked when the user confirms the action.
    pub on_confirm: EventHandler<()>,
    /// Invoked when the user cancels (button or backdrop click).
    pub on_cancel: EventHandler<()>,
}

/// Blocking confirmation overlay. See the module docs for the mounting
/// contract (positioned ancestor) and touch-target guarantees.
///
/// Touch targets: both action buttons are at least 44×44 logical pixels
/// (`min-height: 44px`, padded width well past 44px).
#[component]
pub fn AtConfirmDialog(props: AtConfirmDialogProps) -> Element {
    let confirm_border = if props.danger {
        COLOR_STATUS_ERROR_BORDER
    } else {
        COLOR_ACCENT_PRIMARY
    };
    let button_base = format!(
        "min-height: {th}px; box-sizing: border-box; \
         padding: {py}px {px}px; border-radius: {r}px; \
         font-family: {font}; font-size: {fs}px; font-weight: {fw}; \
         display: flex; align-items: center; justify-content: center;",
        th = TOUCH_MIN,
        py = SPACE_2,
        px = SPACE_4,
        r = RADIUS_MD,
        font = FONT_FAMILY_UI,
        fs = FONT_SIZE_BODY,
        fw = FONT_WEIGHT_SEMIBOLD,
    );

    rsx! {
        // Backdrop: dims and click-blocks the area behind the dialog;
        // clicking it is a cancel. Centres the card via flex.
        div {
            style: "position: absolute; top: 0; left: 0; width: 100%; height: 100%; \
                    z-index: 2000; background: rgba(0, 0, 0, 0.45); \
                    display: flex; align-items: center; justify-content: center;",
            role: "presentation",
            onclick: move |_| props.on_cancel.call(()),

            // The dialog card. Clicks inside must not fall through to the
            // backdrop's cancel handler.
            div {
                style: format!(
                    "width: {w}px; max-width: 90%; box-sizing: border-box; \
                     display: flex; flex-direction: column; gap: {gap}px; \
                     background: {bg}; border: 1px solid {border}; \
                     border-radius: {r}px; padding: {pad}px; \
                     font-family: {font}; color: {fg};",
                    w = DIALOG_WIDTH_PX,
                    gap = SPACE_3,
                    bg = COLOR_SURFACE_1,
                    border = COLOR_BORDER_CHROME,
                    r = RADIUS_MD,
                    pad = SPACE_4,
                    font = FONT_FAMILY_UI,
                    fg = COLOR_TEXT_ON_CHROME,
                ),
                role: "dialog",
                "aria-label": props.title.clone(),
                onclick: move |evt| evt.stop_propagation(),

                // Title
                div {
                    style: format!(
                        "font-size: {fs}px; font-weight: {fw};",
                        fs = FONT_SIZE_MD,
                        fw = FONT_WEIGHT_SEMIBOLD,
                    ),
                    {props.title.clone()}
                }

                // Message body
                div {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = FONT_SIZE_BODY,
                        fg = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    {props.message.clone()}
                }

                // Action row: Cancel (safe, left) then Confirm (destructive).
                div {
                    style: format!(
                        "display: flex; justify-content: flex-end; gap: {gap}px;",
                        gap = SPACE_2,
                    ),
                    button {
                        style: format!(
                            "{button_base} background: transparent; \
                             border: 1px solid {border}; color: {fg};",
                            border = COLOR_BORDER_CHROME,
                            fg = COLOR_TEXT_ON_CHROME,
                        ),
                        onclick: move |evt| {
                            evt.stop_propagation();
                            props.on_cancel.call(());
                        },
                        {props.cancel_label.clone()}
                    }
                    button {
                        style: format!(
                            "{button_base} background: transparent; \
                             border: 1px solid {confirm_border}; color: {confirm_border};",
                        ),
                        onclick: move |evt| {
                            evt.stop_propagation();
                            props.on_confirm.call(());
                        },
                        {props.confirm_label.clone()}
                    }
                }
            }
        }
    }
}
