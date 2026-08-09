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
/// # The baseline moves here, because "do not reorder these" is not a mechanism
///
/// The dirty tracker reads `dirty = live_gen != baseline_gen`, so advancing the
/// mirror without also moving the baseline makes a freshly opened document
/// present as unsaved. This used to be a **written instruction** — the caller
/// recorded `baseline_gen` after this returned, under a doc comment saying "do
/// not reorder those two".
///
/// The instruction was followed and the defect happened anyway, which is the
/// argument against instructions (L08-043). The caller set the baseline inside
/// the `Ok(l_doc)` arm of `document_to_loro`, so a **bridge-init failure** left
/// the mirror advanced and the baseline at `0`: an untouched document, just
/// opened, reporting unsaved changes and offering to save over the file. Nothing
/// was reordered; the second statement was simply on a branch the first was not.
///
/// So both values move in one place, from one read of the published generation.
/// "Advanced the mirror without the baseline" is no longer expressible.
pub(super) fn publish_seed_and_mirror(
    doc_state: &Arc<Mutex<DocumentState>>,
    doc: &Document,
    layout: LaidOut,
    mut targets: SeedTargets,
) -> usize {
    let page_count = publish_seed_layout(doc_state, doc, layout);
    let published = doc_state.lock().ok().map(|state| state.generation);
    if let Some(generation) = published {
        targets.cursor_state.write().document_generation = generation;
        // A freshly published seed *is* the file on disk, on every path that
        // reaches here — including the ones that go on to fail.
        targets.baseline_gen.set(generation);
    }
    page_count
}

/// The two counters a seed publish has to move, carried together.
///
/// A struct rather than two parameters because the pair *is* the invariant: the
/// dirty flag is `live_gen != baseline_gen`, so a caller holding one without the
/// other has nothing useful. Passing them separately is what let the baseline
/// end up on a branch the mirror was not on.
#[derive(Clone, Copy)]
pub(super) struct SeedTargets {
    /// The reactive mirror of `DocumentState::generation`.
    pub(super) cursor_state: Signal<CursorState>,
    /// The generation that matches the file on disk.
    pub(super) baseline_gen: Signal<u64>,
}

#[cfg(test)]
#[path = "editor_seed_publish_tests.rs"]
mod tests;
