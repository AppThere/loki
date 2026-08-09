// SPDX-License-Identifier: Apache-2.0

//! The page family's one way to run a mutation and put the result on screen.
//!
//! # Why this exists
//!
//! Every page-style control — preset, rename, custom size, margin entry, and
//! the three manager verbs — performed the same six steps by hand: take the
//! Loro read guard, bail if there is no document, run the mutation, relayout,
//! release the guard, sync undo/redo. Seven copies of an obligation is seven
//! chances to get one wrong, and one already was: [`super::page_form`]'s preset
//! handler called `post_mutation_sync` **without** releasing the guard first,
//! while its three neighbours released it. Harmless as it happens — nested
//! `Signal::read` is permitted, and the sync only reads — but it is the kind of
//! difference that is invisible until the sync one day needs to write.
//!
//! So this is rule 5: the obligation comes with the capability. There is no way
//! to get the `LoroDoc` from here without the relayout and the sync happening
//! afterwards, in that order, with the guard already gone.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::MutationError;
use loro::LoroDoc;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::StyleEditorSync;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Runs `mutate` against the live document, then relayouts and syncs undo/redo.
///
/// Returns `true` when the mutation ran and the document was updated — callers
/// use it to gate the follow-up that only makes sense if it did, such as moving
/// the panel's selection to a style that now exists.
///
/// Returns `false` without side effects when there is no open document or the
/// mutation errored. A mutation that declines its work (every one of these
/// treats an unknown name as a no-op) returns `Ok`, so it reports `true` — this
/// answers "did the pipeline run", not "did anything change".
pub(super) fn commit(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: StyleEditorSync,
    mutate: impl FnOnce(&LoroDoc) -> Result<(), MutationError>,
) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    if mutate(ldoc).is_err() {
        return false;
    }
    apply_mutation_and_relayout(doc_state, ldoc);
    // Released before the sync, which reads the same signal. Uniformly, here,
    // rather than at six call sites of which one forgot.
    drop(guard);
    post_mutation_sync(
        doc_state,
        sync.loro_doc,
        sync.cursor_state,
        sync.undo_manager,
        sync.can_undo,
        sync.can_redo,
    );
    true
}
