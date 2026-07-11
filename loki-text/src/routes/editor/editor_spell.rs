// SPDX-License-Identifier: Apache-2.0

//! Spell-check editor actions behind the suggestions panel.
//!
//! Resolves the word under a document position, applies a chosen suggestion
//! (delete + insert via the Loro mutation layer), and handles the
//! personal-dictionary / ignore / language changes — each of which re-checks the
//! document by refreshing the ambient layout spell state and forcing a full
//! relayout so squiggles update.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_app_shell::spell::{Consent, ReqwestFetcher, SpellService};
use loki_doc_model::loro_mutation::{get_block_text, replace_text};

use crate::editing::cursor::CursorState;
use crate::editing::relayout::{page_metrics, relayout_paginated};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use crate::editing::touch::word_boundaries_at;
use crate::routes::editor::editor_keydown_ctrl::post_mutation_sync;

/// The word a right-click resolved, with its spelling status and suggestions.
#[derive(Clone, PartialEq)]
pub(super) struct SpellMenu {
    pub paragraph_index: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub word: String,
    pub misspelled: bool,
    pub suggestions: Vec<String>,
    /// Client (window-relative) coordinates of the right-click, used to anchor
    /// the floating menu at the cursor.
    pub anchor_x: f32,
    pub anchor_y: f32,
}

/// Editor signal handles needed to apply a spelling mutation, bundled to keep
/// call sites short (mirrors the other panels' `*Sync` structs).
#[derive(Clone, Copy)]
pub(super) struct SpellSync {
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub cursor_state: Signal<CursorState>,
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    pub can_undo: Signal<bool>,
    pub can_redo: Signal<bool>,
}

/// Resolves the word at `(paragraph_index, byte_offset)` into a [`SpellMenu`],
/// or `None` if there is no word there.
pub(super) fn resolve_spell_menu(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    service: &SpellService,
    paragraph_index: usize,
    byte_offset: usize,
) -> Option<SpellMenu> {
    let guard = loro_doc.read();
    let ldoc = guard.as_ref()?;
    let text = get_block_text(ldoc, paragraph_index);
    let (byte_start, byte_end) = word_boundaries_at(&text, byte_offset)?;
    let word = text.get(byte_start..byte_end)?.to_string();
    if word.trim().is_empty() {
        return None;
    }
    let misspelled = !service.is_correct(&word);
    let suggestions = if misspelled {
        service.suggest(&word)
    } else {
        Vec::new()
    };
    Some(SpellMenu {
        paragraph_index,
        byte_start,
        byte_end,
        word,
        misspelled,
        suggestions,
        // The caller fills these from the click position.
        anchor_x: 0.0,
        anchor_y: 0.0,
    })
}

/// Replaces the menu's word with `replacement` as a single undoable edit.
pub(super) fn replace_word(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: SpellSync,
    menu: &SpellMenu,
    replacement: &str,
) {
    {
        let guard = sync.loro_doc.read();
        if let Some(ldoc) = guard.as_ref() {
            let len = menu.byte_end - menu.byte_start;
            // `replace_text` preserves the replaced word's character formatting
            // (a plain delete+insert would let an adjacent run's colour bleed in).
            let _ = replace_text(
                ldoc,
                menu.paragraph_index,
                menu.byte_start,
                len,
                replacement,
            );
            apply_mutation_and_relayout(doc_state, ldoc);
        }
    }
    post_mutation_sync(
        doc_state,
        sync.loro_doc,
        sync.cursor_state,
        sync.undo_manager,
        sync.can_undo,
        sync.can_redo,
    );
}

/// Adds `word` to the personal dictionary and re-checks the document.
pub(super) fn add_to_dictionary(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    service: &SpellService,
    word: &str,
) {
    service.add_word(word);
    refresh_and_relayout(doc_state, cursor_state, service);
}

/// Ignores `word` for the session and re-checks the document.
pub(super) fn ignore_word(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    service: &SpellService,
    word: &str,
) {
    service.ignore_word(word);
    refresh_and_relayout(doc_state, cursor_state, service);
}

/// Activates an already-available (bundled or installed) language and re-checks.
/// Returns `false` if the dictionary failed to load.
pub(super) fn activate_language(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    service: &SpellService,
    tag: &str,
) -> bool {
    if service.activate_language(tag).is_err() {
        return false;
    }
    refresh_and_relayout(doc_state, cursor_state, service);
    true
}

/// Downloads, installs, activates `tag`, then re-checks. Runs the blocking
/// network install on a worker thread so the UI loop is never blocked; the
/// relayout happens back on the UI task after the download completes.
pub(super) async fn download_and_activate(
    doc_state: Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    service: SpellService,
    tag: String,
    consent: Consent,
) -> bool {
    let (tx, rx) = futures_channel::oneshot::channel();
    let worker_service = service.clone();
    let worker_tag = tag.clone();
    if std::thread::Builder::new()
        .name("loki-dict-download".into())
        .spawn(move || {
            let ok = match ReqwestFetcher::new() {
                Ok(fetcher) => worker_service
                    .install_and_activate(&worker_tag, consent, &fetcher)
                    .is_ok(),
                Err(_) => false,
            };
            let _ = tx.send(ok);
        })
        .is_err()
    {
        return false;
    }
    let ok = rx.await.unwrap_or(false);
    if ok {
        refresh_and_relayout(&doc_state, cursor_state, &service);
    }
    ok
}

/// Pushes the service's current checker into the renderer's ambient spell state
/// and forces a full relayout so squiggles reflect the change.
fn refresh_and_relayout(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    service: &SpellService,
) {
    loki_renderer::spell::set_active(service.snapshot().map(|snap| loki_layout::SpellState {
        // Active dictionary registered under its own tag → per-run routing
        // skips runs tagged with languages we have no dictionary for (gap #30).
        checkers: std::iter::once((snap.language, snap.checker.clone())).collect(),
        checker: snap.checker,
        generation: snap.generation,
    }));
    force_full_relayout(doc_state, cursor_state);
}

/// Re-lays-out the current document in full (no incremental reuse) and bumps the
/// generation so the canvas repaints. Used for dictionary/language changes that
/// alter squiggles without editing the document text.
fn force_full_relayout(
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
) {
    let (fr_arc, doc) = {
        let Ok(state) = doc_state.lock() else { return };
        (state.shared_font_resources.clone(), state.document.clone())
    };
    let Some(doc) = doc else { return };
    let laid_out = {
        let mut fr = fr_arc.lock().unwrap_or_else(|e| e.into_inner());
        // `None` previous → full pass, so every paragraph re-checks under the new
        // generation rather than reusing cached pages.
        relayout_paginated(&mut fr, &doc, None)
    };
    let (page_count, width_px, height_px) = page_metrics(&laid_out.layout);
    // Publish a *fresh* document `Arc` (same content). The renderer's
    // `DocPageSource::update_doc` compares by `Arc` pointer and only recomputes
    // the painted layout when the pointer changes — and a dictionary change does
    // not touch the document — so without a new `Arc` the squiggles would not
    // repaint. Dictionary/language changes are rare, so the clone is cheap enough.
    let fresh_doc = Arc::new((*doc).clone());
    let generation = {
        let Ok(mut state) = doc_state.lock() else {
            return;
        };
        state.document = Some(fresh_doc);
        state.paginated_layout = Some(Arc::new(laid_out.layout));
        state.layout_reuse = Some(laid_out.reuse);
        state.page_count = page_count;
        state.page_width_px = width_px;
        state.page_height_px = height_px;
        state.generation = state.generation.wrapping_add(1);
        state.generation
    };
    cursor_state.write().document_generation = generation;
}
