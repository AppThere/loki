// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The zoom control's translated labels, for all three suite apps (Spec 08
//! T5.4).
//!
//! # One derivation, because it is one fact
//!
//! `AtZoomLabels` has seven fields and every app fills them from the same seven
//! Fluent keys. Written at each call site that is three copies of one table:
//! adding a row means editing three files, and the one that gets missed shows a
//! blank label rather than failing to compile. This is evidence rule 4 applied
//! before the drift rather than after it — and it is why the helper lives here,
//! in the crate all three already share, rather than in `appthere_ui`, which is
//! deliberately i18n-agnostic.

use appthere_ui::AtZoomLabels;
use loki_i18n::fl;

/// The zoom control's labels in the active locale.
#[must_use]
pub fn zoom_labels() -> AtZoomLabels {
    AtZoomLabels {
        zoom_out: fl!("editor-zoom-out"),
        zoom_in: fl!("editor-zoom-in"),
        menu: fl!("editor-zoom-menu"),
        // Supplied even by apps whose `ZoomCommands` hide these rows: a label is
        // not a capability, and an empty string here would become the visible
        // bug on the day an app gains the row.
        fit_width: fl!("editor-zoom-fit-width"),
        fit_page: fl!("editor-zoom-fit-page"),
        actual_size: fl!("editor-zoom-actual-size"),
        reduced_note: fl!("editor-zoom-reduced"),
    }
}
