// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The editor's display-calibration flow (Spec 08 T5.5, D-04).
//!
//! # A component, not a function called inside an `if` (ADR-0013)
//!
//! It owns hooks — the calibration store is loaded once here rather than on
//! every render of the editor — so only a component can host it, and it is
//! mounted at the boundary by `editor_inner`.
//!
//! # What it does with the answer
//!
//! Turns the reader's measurement into a density (`calibrated_css_ppi`, which
//! refuses one it cannot believe), records it against this display, saves, and
//! notes it on the profile as **calibrated** — which is what stops the platform
//! probe from overwriting it on the next observation.

use appthere_ui::scroll::ZoomAnchor;
use appthere_ui::{
    AtCalibrateDialog, AtCalibrateLabels, REFERENCE_MM, actual_size_zoom_percent,
    note_display_density,
};
use dioxus::prelude::*;
use loki_app_shell::display_calibration::DisplayCalibrations;
use loki_app_shell::display_density::calibrated_css_ppi;
use loki_i18n::fl;

use super::editor_zoom::ZoomCommand;

/// The density the reference line is drawn at when nothing is known.
///
/// The CSS definition of a pixel — 1/96 inch — which is exactly the assumption
/// the measurement is correcting. Starting anywhere else would draw a line whose
/// nominal length the app has no reason to believe.
const ASSUMED_CSS_PPI: f32 = 96.0;

/// Props for [`EditorCalibrate`].
#[derive(Props, Clone, PartialEq)]
pub(super) struct EditorCalibrateProps {
    /// Cleared when the dialog closes, either way.
    pub open: Signal<bool>,
    /// The zoom command, used to apply Actual Size once a measurement lands.
    pub zoom: ZoomCommand,
}

/// Hosts the calibration dialog and stores its result.
#[component]
pub(super) fn EditorCalibrate(props: EditorCalibrateProps) -> Element {
    let mut open = props.open;
    let zoom = props.zoom;
    // Loaded once per mount rather than per render: this touches the disk.
    let store = use_signal(DisplayCalibrations::load);

    // The display being calibrated, from the same derivation the restore path
    // uses — a measurement saved under one key and looked up under another reads
    // as the app forgetting. `current_display_key` names a real display where a
    // platform query exists and `unidentified` where none does; see its docs.
    let key = use_hook(crate::device_probe::current_display_key);

    rsx! {
        AtCalibrateDialog {
            assumed_css_ppi: ASSUMED_CSS_PPI,
            labels: AtCalibrateLabels {
                title: fl!("editor-calibrate-title"),
                // Formatted here, not handed to Fluent as a number: `f64::from(85.6f32)`
                // is 85.5999984741211, and the first screen sitting put exactly that
                // on the dialog. The reference is a length a person reads off a
                // ruler, so it is presented to a ruler's precision.
                instructions: fl!(
                    "editor-calibrate-instructions",
                    mm = format!("{REFERENCE_MM:.1}")
                ),
                field_label: fl!("editor-calibrate-field"),
                apply: fl!("editor-calibrate-apply"),
                cancel: fl!("editor-calibrate-cancel"),
                rejected: fl!("editor-calibrate-rejected"),
            },
            on_measured: move |mm: f32| -> bool {
                // A measurement the arithmetic refuses leaves the dialog open
                // **and says so** — the `false` is what raises the dialog's
                // rejection notice. Returning silently, which this did until the
                // branch review, left the reader with a button that did nothing:
                // the dialog had already cleared the notice on the way in,
                // believing any positive number.
                let Some(density) = calibrated_css_ppi(ASSUMED_CSS_PPI, REFERENCE_MM, mm) else {
                    return false;
                };
                let mut store = store;
                store.write().set(key.clone(), density);
                store.read().save();
                note_display_density(density.css_px_per_inch, true);
                // **Finish the command the reader actually gave.** They chose
                // Actual Size; calibrating was the means. Closing here without
                // applying would leave the page at whatever zoom it was, having
                // asked them to fetch a ruler — and they would have to choose
                // Actual Size a second time to see any effect.
                if let Some(p) = actual_size_zoom_percent(Some(density.css_px_per_inch)) {
                    zoom.set(p, ZoomAnchor::Centre);
                }
                open.set(false);
                true
            },
            on_cancel: move |()| open.set(false),
        }
    }
}
