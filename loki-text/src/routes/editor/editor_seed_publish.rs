// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Publishing a freshly-opened document's seed layout, and mirroring the
//! generation that publish produced into the reactive view of it (Spec 08 I-10).

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::document::Document;

use crate::editing::cursor::CursorState;
use crate::editing::relayout::LaidOut;
use crate::editing::state::{DocumentState, publish_seed_layout};

/// Publishes the seed layout and mirrors the resulting content generation into
/// `cursor_state`, returning the page count.
///
/// # Why the mirror is here and not in the word count (I-10's root cause)
///
/// [`DocumentState::generation`] is the authoritative content counter, and both
/// the edit path (`apply_mutation_and_relayout`) and this open path bump it. But
/// nothing can *subscribe* to a field behind a `Mutex`, so the reactive view of it
/// is the cursor's mirrored `document_generation` — and until r19 only the edit
/// paths wrote that mirror.
///
/// So on a fresh open the real counter advanced and the reactive one did not.
/// Everything watching the mirror concluded nothing had happened; the status-bar
/// word count, whose memo had run once at mount while `state.document` was still
/// `None`, kept publishing that mount-time `0` until the first keystroke. That is
/// the whole of I-10.
///
/// Fixing it at the mirror rather than in the word count's dependency list is
/// deliberate: the mirror was **incomplete**, so every consumer of it was equally
/// stale on open, and patching one consumer would have left the others wrong while
/// looking like the bug was fixed.
///
/// # Ordering constraint, which is load-bearing
///
/// The dirty tracker reads `dirty = live_gen != baseline_gen`, so advancing the
/// mirror without also moving the baseline would make every freshly opened
/// document present as unsaved. The caller records `baseline_gen` from the mirror
/// *after* this returns, which keeps them equal. **Do not reorder those two.**
pub(super) fn publish_seed_and_mirror(
    doc_state: &Arc<Mutex<DocumentState>>,
    doc: &Document,
    layout: LaidOut,
    mut cursor_state: Signal<CursorState>,
) -> usize {
    let page_count = publish_seed_layout(doc_state, doc, layout);
    let published = doc_state.lock().ok().map(|state| state.generation);
    if let Some(generation) = published {
        cursor_state.write().document_generation = generation;
    }
    page_count
}

#[cfg(test)]
#[path = "editor_seed_publish_tests.rs"]
mod tests;
