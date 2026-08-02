// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtStatusBarProps`] — split from `status_bar.rs` when the zoom badge became
//! the zoom control (Spec 08 T5.4) and the props outgrew the 300-line ceiling
//! alongside the component.

use dioxus::prelude::*;

use crate::components::zoom_control::{AtZoomLabels, ZoomCommands};

#[derive(Props, Clone, PartialEq)]
/// Props for [`super::status_bar::AtStatusBar`].
pub struct AtStatusBarProps {
    /// Pre-formatted page label, e.g. `"Page 1 of 4"`.
    pub page_label: String,

    /// Pre-formatted word count label, e.g. `"1,847 words"`.
    pub word_count_label: String,

    /// Active language label, e.g. `"English (US)"`.
    pub language_label: String,

    /// The zoom the user **requested**, in percent.
    ///
    /// Requested, not effective: Spec 08 T5.4 requirement 6. A status bar that
    /// showed the capability-capped figure would erode the reader's setting one
    /// adjustment at a time, since each nudge would start from the reduced
    /// number. `capability_limit_permille` is what keeps that display honest.
    pub zoom_percent: u32,

    /// The capability cap in force, in permille, if any — drives the "reduced"
    /// indicator beside the readout.
    #[props(default)]
    pub zoom_capability_limit_permille: Option<u16>,

    /// Which computed zoom rows this app can service.
    #[props(default)]
    pub zoom_commands: ZoomCommands,

    /// Translated labels for the zoom control.
    pub zoom_labels: AtZoomLabels,

    /// A new requested zoom, in percent.
    pub on_zoom_change: EventHandler<u32>,

    /// Fit-width chosen. Only reachable when `zoom_commands.fit_width`.
    #[props(default)]
    pub on_zoom_fit_width: EventHandler<()>,

    /// Fit-page chosen. Only reachable when `zoom_commands.fit_page`.
    #[props(default)]
    pub on_zoom_fit_page: EventHandler<()>,

    /// Actual-size chosen. Only reachable when `zoom_commands.actual_size`.
    #[props(default)]
    pub on_zoom_actual_size: EventHandler<()>,

    /// Number of active remote collaborators. `0` = hide the collaborator badge.
    pub collaborator_count: u32,

    /// Pre-formatted collaborator label, e.g. `"2 connected"`.
    /// Only rendered when `collaborator_count > 0`.
    pub collaborator_label: String,

    /// Label for the optional view-mode toggle (e.g. `"Paginated"`/`"Reflowed"`).
    /// Empty (the default) hides the toggle, so apps that do not offer it are
    /// unaffected.
    #[props(default)]
    pub view_mode_label: String,

    /// Aria label for the view-mode toggle button.
    #[props(default)]
    pub view_mode_aria_label: String,

    /// Callback invoked when the view-mode toggle is clicked. Defaults to a
    /// no-op when not provided.
    #[props(default)]
    pub on_view_mode_click: Callback<()>,

    /// Optional status-notice chip rendered on the left (e.g. the recovery
    /// affordance for a dismissed font-substitution warning). Empty (the
    /// default) hides it, so apps that do not use it are unaffected. Generic by
    /// design — not font-specific.
    ///
    /// Touch target: ≥ `TOUCH_MIN` wide × full bar height (see the component
    /// doc for the shared status-bar-height constraint).
    #[props(default)]
    pub notice_label: String,

    /// Aria label for the notice chip.
    #[props(default)]
    pub notice_aria_label: String,

    /// Callback invoked when the notice chip is clicked.
    #[props(default)]
    pub on_notice_click: Callback<()>,

    /// Optional transient status chip (e.g. "Document saved"). Empty (the
    /// default) hides it. The app owns the message's lifetime — auto-clearing
    /// and clear-on-edit live in the caller; clicking the chip dismisses it.
    ///
    /// Touch target: ≥ `TOUCH_MIN` wide × full bar height.
    #[props(default)]
    pub status_note_label: String,

    /// Callback invoked when the status chip is clicked (dismiss).
    #[props(default)]
    pub on_status_note_click: Callback<()>,
}
