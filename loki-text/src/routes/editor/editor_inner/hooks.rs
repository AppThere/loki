// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reactive effect closures for the document editor.
//!
//! Each public function returns a closure suitable for passing to
//! `use_effect`.  Extracting them here keeps the component body under the
//! 300-line file ceiling while preserving identical runtime behaviour.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_doc_model::get_mark_at;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_doc_model::loro_schema::{
    MARK_BOLD, MARK_ITALIC, MARK_STRIKETHROUGH, MARK_UNDERLINE, MARK_VERTICAL_ALIGN,
};
use loro::LoroValue;

use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, seed_layout_from_document};
use crate::error::LoadError;

/// Returns the closure for the Loro-bridge initialisation effect.
///
/// Runs once after the document resource resolves.  Seeds the layout cache
/// and initialises the Loro CRDT + undo manager.
pub(super) fn make_loro_init_effect(
    doc_state_seed: Arc<Mutex<DocumentState>>,
    mut loro_doc: Signal<Option<loro::LoroDoc>>,
    mut undo_manager: Signal<Option<loro::UndoManager>>,
    mut cursor_state: Signal<CursorState>,
    path_signal: Signal<String>,
    document_load: Resource<(String, Result<Document, LoadError>)>,
) -> impl FnMut() {
    move || {
        if let Some((loaded_path, Ok(doc))) = &*document_load.value().read_unchecked()
            && loaded_path == &path_signal()
            && loro_doc().is_none()
        {
            seed_layout_from_document(&doc_state_seed, doc);
            match document_to_loro(doc) {
                Ok(l_doc) => {
                    let um = loro::UndoManager::new(&l_doc);
                    loro_doc.set(Some(l_doc));
                    undo_manager.set(Some(um));

                    if cursor_state.read().focus.is_none() {
                        let start = DocumentPosition {
                            page_index: 0,
                            paragraph_index: 0,
                            byte_offset: 0,
                        };
                        let mut cs = cursor_state.write();
                        cs.anchor = Some(start.clone());
                        cs.focus = Some(start);
                    }
                }
                Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
            }
        }
    }
}

/// Returns the closure for the page-count synchronisation effect.
///
/// Subscribes to the document-load resource so it re-runs when loading
/// completes and `doc_state.page_count` is updated.
pub(super) fn make_page_count_effect(
    doc_state_pages: Arc<Mutex<DocumentState>>,
    document_load: Resource<(String, Result<Document, LoadError>)>,
    mut total_pages: Signal<u32>,
) -> impl FnMut() {
    move || {
        let resource_signal = document_load.value();
        let _sub = resource_signal.read();
        if let Ok(state) = doc_state_pages.lock() {
            let count = state.page_count as u32;
            if *total_pages.peek() != count {
                total_pages.set(count);
            }
        }
    }
}

/// Returns the closure for the inline-formatting signal-sync effect.
///
/// Subscribes to `cursor_state` and `loro_doc` so it re-runs whenever the
/// cursor moves or the document changes, keeping ribbon button active-states
/// in sync.
// Pre-existing pattern — structural refactor deferred; signals can't be bundled into a struct
#[allow(clippy::too_many_arguments)]
pub(super) fn make_formatting_effect(
    cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut bold_active: Signal<bool>,
    mut italic_active: Signal<bool>,
    mut underline_active: Signal<bool>,
    mut strikethrough_active: Signal<bool>,
    mut superscript_active: Signal<bool>,
    mut subscript_active: Signal<bool>,
) -> impl FnMut() {
    move || {
        let cs = cursor_state.read();
        let ldoc_guard = loro_doc.read();
        if let (Some(ldoc), Some(focus)) = (ldoc_guard.as_ref(), cs.focus.as_ref()) {
            let bi = focus.paragraph_index;
            let bo = focus.byte_offset;
            let is_bool = |key: &str| {
                matches!(
                    get_mark_at(ldoc, bi, bo, key),
                    Ok(Some(LoroValue::Bool(true)))
                )
            };
            bold_active.set(is_bool(MARK_BOLD));
            italic_active.set(is_bool(MARK_ITALIC));
            underline_active.set(is_bool(MARK_UNDERLINE));
            strikethrough_active.set(is_bool(MARK_STRIKETHROUGH));
            superscript_active.set(matches!(
                get_mark_at(ldoc, bi, bo, MARK_VERTICAL_ALIGN),
                Ok(Some(LoroValue::String(ref s))) if s.as_str() == "Superscript"
            ));
            subscript_active.set(matches!(
                get_mark_at(ldoc, bi, bo, MARK_VERTICAL_ALIGN),
                Ok(Some(LoroValue::String(ref s))) if s.as_str() == "Subscript"
            ));
        } else {
            bold_active.set(false);
            italic_active.set(false);
            underline_active.set(false);
            strikethrough_active.set(false);
            superscript_active.set(false);
            subscript_active.set(false);
        }
    }
}
