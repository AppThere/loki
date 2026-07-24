// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Macro-security dialogs (macro spec §2.3, §5.4, §5.5).
//!
//! Two overlays, both rendered in the anti-spoof [`MacroDialogFrame`] so
//! macro-originated UI can never impersonate app chrome (threat T7):
//!
//! - [`AtMacroTrustDialog`] — the three-choice enable dialog (§2.3);
//! - [`AtPermissionPrompt`] — a first-use capability prompt (§5.4).
//!
//! Both are `appthere_ui`-pure: they take display strings as props and emit an
//! abstract choice enum, so the crate stays free of any macro-host or document
//! dependency (the hosting app maps the choice to a `MacroService` call).

mod frame;
mod network;
mod permission;
mod trust;

pub use frame::{MacroDialogFrame, MacroDialogFrameProps};
pub use network::{AtNetworkPrompt, AtNetworkPromptProps};
pub use permission::{AtPermissionPrompt, AtPermissionPromptProps};
pub use trust::{AtMacroTrustDialog, AtMacroTrustDialogProps};

use crate::tokens::colors::{COLOR_MACRO_BADGE, COLOR_SURFACE_3, COLOR_TEXT_ON_CHROME};
use crate::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_WEIGHT_SEMIBOLD};

/// The user's answer to the enable dialog (spec §2.3). The hosting app maps this
/// onto the matching `MacroService` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroTrustChoice {
    /// Keep macros disabled (sticky). The safe default; backdrop maps here.
    KeepDisabled,
    /// Enable for this session only (not persisted).
    EnableSession,
    /// Persistently trust this document.
    TrustAlways,
}

/// The user's answer to a first-use capability prompt (spec §5.4). Maps onto a
/// `GrantScope` in the hosting app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroGrantChoice {
    /// Refuse (default) — the script sees a trappable error.
    Deny,
    /// Allow for this run only.
    AllowOnce,
    /// Allow until the document closes.
    AllowSession,
    /// Allow and persist to the document's trust record.
    AlwaysForDocument,
}

/// Style for a full-width choice button in a macro dialog. `accented` draws the
/// macro-badge border to mark the strongest (most-granting) option.
///
/// Touch target: `min-height: TOUCH_MIN` (44 px) with generous padding meets
/// WCAG 2.5.8.
#[must_use]
pub(crate) fn choice_button_style(accented: bool) -> String {
    let border = if accented {
        COLOR_MACRO_BADGE
    } else {
        COLOR_SURFACE_3
    };
    format!(
        "width: 100%; min-height: {th}px; box-sizing: border-box; \
         padding: {py}px {px}px; border-radius: {r}px; \
         background: transparent; border: 1px solid {border}; \
         color: {fg}; font-family: {font}; font-size: {fs}px; \
         font-weight: {fw}; cursor: pointer; text-align: center; \
         display: flex; align-items: center; justify-content: center;",
        th = TOUCH_MIN,
        py = SPACE_2,
        px = SPACE_3,
        r = RADIUS_SM,
        border = border,
        fg = COLOR_TEXT_ON_CHROME,
        font = FONT_FAMILY_UI,
        fs = FONT_SIZE_BODY,
        fw = FONT_WEIGHT_SEMIBOLD,
    )
}
