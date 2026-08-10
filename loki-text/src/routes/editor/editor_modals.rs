// SPDX-License-Identifier: Apache-2.0

//! The editor's modal overlays, mounted as one cluster.
//!
//! # Why these live together, and not in `editor_inner`
//!
//! Every modal here is mounted **at a component boundary** — `{cond.then(|| rsx!
//! { Modal { .. } })}` — because only a component owns a hook scope, so only a
//! component can read the breakpoint and adapt without a `compact` flag threaded
//! down from the parent (ADR-0013). Each is a few lines at the mount site and
//! grows whenever a modal gains a prop, and `editor_inner` is baselined over the
//! 300-line ceiling and may not grow — so the cluster lives here and
//! `editor_inner` calls it once (CLAUDE.md technique 3).
//!
//! # The mounting contract
//!
//! Each overlay paints a `position: absolute` full-area backdrop, so **the
//! caller's container must be `position: relative` and span the area to dim**.
//! `editor_inner`'s root div is exactly that. (`position: fixed` collapses to
//! `absolute` in `stylo_taffy` and must not be used — root `CLAUDE.md`.)

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use super::editor_style_editor::StyleEditorSync;
use super::style_dialog::{ParagraphStyleDialog, ParagraphStyleDialogProps};
use crate::editing::state::DocumentState;

/// Renders whichever modal overlays are currently open.
///
/// Every argument is a signal or a shared handle, so this is a plain function:
/// it hosts no hooks of its own, and each modal it mounts is a real component
/// that hosts its own.
pub(super) fn editor_modals(
    doc_state: Arc<Mutex<DocumentState>>,
    // Display-calibration dialog gate (Spec 08 T5.5 / D-04).
    calibrating: Signal<bool>,
    // Zoom command sink the calibration dialog writes through.
    zoom: super::editor_zoom::ZoomCommand,
    // The paragraph style being edited in the tabbed dialog; `None` = closed.
    paragraph_style_dialog: Signal<Option<String>>,
    // Font families enumerated on this device (memoised by the caller).
    font_families: Rc<Vec<String>>,
    // Loro / undo plumbing shared with the inline style panel.
    sync: StyleEditorSync,
) -> Element {
    let open_style = paragraph_style_dialog.read().clone();

    rsx! {
        // Display calibration (Spec 08 T5.5 / D-04).
        {calibrating().then(|| rsx! {
            super::editor_calibrate::EditorCalibrate { open: calibrating, zoom }
        })}

        // Paragraph style editor (Spec 05 M2/M6, design section 1).
        {open_style.map(|style_id| rsx! {
            {ParagraphStyleDialog(ParagraphStyleDialogProps {
                doc_state: Arc::clone(&doc_state),
                open_style: paragraph_style_dialog,
                style_id,
                font_families: Rc::clone(&font_families),
                sync,
            })}
        })}
    }
}
