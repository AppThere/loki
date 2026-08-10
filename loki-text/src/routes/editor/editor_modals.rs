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

use super::editor_dialog_state::DialogSignals;
use super::editor_insert_sync::InsertLinkSync;
use super::editor_style_editor::StyleEditorSync;
use super::link_dialog::{InsertLinkDialog, InsertLinkDialogProps};
use super::meta_dialog::{MetadataDialog, MetadataDialogProps};
use super::page_dialog::{PageStyleDialog, PageStyleDialogProps};
use super::publish_dialog::{PublishEpubDialog, PublishEpubDialogProps};
use super::span_dialog::{SpanFormatDialog, SpanFormatDialogProps};
use super::style_dialog::{ParagraphStyleDialog, ParagraphStyleDialogProps};
use super::table_dialog::{InsertTableDialog, InsertTableDialogProps};
use crate::editing::state::DocumentState;

/// Renders whichever modal overlays are currently open.
///
/// Every argument is a signal or a shared handle, so this is a plain function:
/// it hosts no hooks of its own, and each modal it mounts is a real component
/// that hosts its own.
#[allow(clippy::too_many_arguments)]
pub(super) fn editor_modals(
    doc_state: Arc<Mutex<DocumentState>>,
    // Display-calibration dialog gate (Spec 08 T5.5 / D-04).
    calibrating: Signal<bool>,
    // Zoom command sink the calibration dialog writes through.
    zoom: super::editor_zoom::ZoomCommand,
    // The paragraph style being edited in the tabbed dialog; `None` = closed.
    paragraph_style_dialog: Signal<Option<String>>,
    // Open state for the six dialogs of design sections 2–7.
    dialogs: DialogSignals,
    // Font families enumerated on this device (memoised by the caller).
    font_families: Rc<Vec<String>>,
    // The document's path, for the EPUB dialog's suggested output name.
    path: Signal<String>,
    // Loro / undo plumbing shared with the inline style panel.
    sync: StyleEditorSync,
) -> Element {
    let open_style = paragraph_style_dialog.read().clone();
    let open_page = dialogs.page_style.read().clone();
    let link_open = dialogs.insert_link.read().is_some();
    let table_open = dialogs.insert_table.read().is_some();
    // The insert dialogs take the narrower bundle the link panel defined; it is
    // a strict subset of the style editor's, so there is nothing to thread from
    // the caller that is not already here.
    let insert_sync = InsertLinkSync {
        loro_doc: sync.loro_doc,
        cursor_state: sync.cursor_state,
        undo_manager: sync.undo_manager,
        can_undo: sync.can_undo,
        can_redo: sync.can_redo,
    };

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

        // Span-level formatting (design section 2).
        {dialogs.span_format.cloned().then(|| rsx! {
            {SpanFormatDialog(SpanFormatDialogProps {
                doc_state: Arc::clone(&doc_state),
                open: dialogs.span_format,
                font_families: Rc::clone(&font_families),
                sync: insert_sync,
            })}
        })}

        // Page style editor (design section 3).
        {open_page.map(|_| rsx! {
            {PageStyleDialog(PageStyleDialogProps {
                doc_state: Arc::clone(&doc_state),
                open: dialogs.page_style,
                sync,
            })}
        })}

        // Document properties (design section 4).
        {dialogs.metadata.cloned().then(|| rsx! {
            {MetadataDialog(MetadataDialogProps {
                doc_state: Arc::clone(&doc_state),
                open: dialogs.metadata,
                sync,
            })}
        })}

        // Insert link (design section 5).
        {link_open.then(|| rsx! {
            {InsertLinkDialog(InsertLinkDialogProps {
                doc_state: Arc::clone(&doc_state),
                open: dialogs.insert_link,
                sync: insert_sync,
            })}
        })}

        // Insert table (design section 6).
        {table_open.then(|| rsx! {
            {InsertTableDialog(InsertTableDialogProps {
                doc_state: Arc::clone(&doc_state),
                open: dialogs.insert_table,
                sync: insert_sync,
            })}
        })}

        // Publish EPUB 3 (design section 7). Its preflight links to the
        // properties dialog above, so it carries that dialog's open signal.
        {dialogs.publish_epub.cloned().then(|| rsx! {
            {PublishEpubDialog(PublishEpubDialogProps {
                doc_state: Arc::clone(&doc_state),
                open: dialogs.publish_epub,
                path,
                save_message: sync.save_message,
                open_metadata: dialogs.metadata,
            })}
        })}
    }
}
