// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The mutation → relayout → publish path for the text editor (split from
//! `state.rs` for the 300-line ceiling): re-derives the `Document` from Loro
//! (incrementally when possible), runs a paginated relayout reusing unchanged
//! pages, and publishes the new state + generation. `apply_mutation_and_relayout`
//! is re-exported from `state.rs`.

use std::sync::{Arc, Mutex};

use loki_doc_model::loro_bridge::IncrementalReader;

use super::DocumentState;
use crate::editing::relayout::{page_metrics, relayout_paginated};

/// Re-derives the document from `loro_doc`, runs a full layout pass, and
/// publishes the updated state to `doc_state`.
///
/// Call after any `insert_text` / `delete_text` / formatting mutation.
/// Returns `true` on success.
pub fn apply_mutation_and_relayout(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: &loro::LoroDoc,
) -> bool {
    // Step 1+2: Derive the Document from Loro — incrementally re-deriving only
    // the changed block(s) when possible — and restore the style catalog and
    // source from the previously published document (neither is stored in Loro).
    let doc = {
        let Ok(mut state) = doc_state.lock() else {
            tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (derive)");
            return false;
        };
        // Lazily seed the incremental reader against this Loro document.
        if state.incremental.is_none() {
            match IncrementalReader::seed(loro_doc) {
                Ok(reader) => state.incremental = Some(reader),
                Err(e) => {
                    tracing::warn!("apply_mutation_and_relayout: incremental seed failed: {e}");
                    return false;
                }
            }
        }
        let mut doc = match state.incremental.as_mut() {
            Some(reader) => match reader.update(loro_doc) {
                Ok(d) => d.clone(),
                Err(e) => {
                    tracing::warn!("apply_mutation_and_relayout: incremental update failed: {e}");
                    return false;
                }
            },
            None => return false,
        };
        if let Some(orig) = &state.document {
            // `source` is not stored in the CRDT, so carry it forward. Metadata
            // and the style catalog *are* round-tripped through Loro (read back
            // by `loro_to_document`), so they are intentionally not carried
            // forward — the Loro snapshot is the source of truth (style edits undoable).
            doc.source = orig.source.clone();
        }
        doc
    };

    // Step 3: Relayout — incrementally reusing unchanged pages from the previous
    // layout when the edit is eligible, else a full pass. Capture the previous
    // document/layout/reuse (cheap Arc clones) so the heavy layout runs without
    // holding the state lock.
    let (fr_arc, prev_doc, prev_layout, prev_reuse) = {
        let Ok(state) = doc_state.lock() else {
            tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (font)");
            return false;
        };
        (
            state.shared_font_resources.clone(),
            state.document.clone(),
            state.paginated_layout.clone(),
            state.layout_reuse.clone(),
        )
    };
    let laid_out = {
        let mut fr = fr_arc.lock();
        let prev = match (&prev_doc, &prev_layout, &prev_reuse) {
            (Some(d), Some(l), Some(r)) => Some((d.as_ref(), l.as_ref(), r)),
            _ => None,
        };
        relayout_paginated(&mut fr, &doc, prev)
    };
    let (page_count, page_width_px, page_height_px) = page_metrics(&laid_out.layout);
    let incremental = laid_out.incremental;

    // Step 4: Publish.
    let block_count: usize = doc.sections.iter().map(|s| s.blocks.len()).sum();
    let Ok(mut state) = doc_state.lock() else {
        tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (publish)");
        return false;
    };
    state.document = Some(Arc::new(doc));
    state.paginated_layout = Some(Arc::new(laid_out.layout));
    state.layout_reuse = Some(laid_out.reuse);
    state.font_substitutions.extend(laid_out.substitutions);
    state.page_count = page_count;
    state.page_width_px = page_width_px;
    state.page_height_px = page_height_px;
    state.generation = state.generation.wrapping_add(1);
    drop(state);

    log_memory_counters(loro_doc, &fr_arc, page_count, block_count, incremental);
    true
}

/// Throttled, opt-in memory instrumentation for the edit session.
///
/// The Loro oplog and rich-text tombstones grow with edit history and never
/// auto-compact (see `docs/memory-audit-2026-06-12.md`, Finding 6), so a long
/// editing session can balloon resident memory. Because the headless build has
/// no profiler, this logs the cheap Loro op/change counters (and the document's
/// stable page/block counts for contrast) on the edit path so the grower can be
/// identified on-device:
///
/// ```text
/// RUST_LOG=loki_text::mem=info cargo run -p loki-text --release
/// ```
///
/// One log line answers the usage audit's §15 Phase-1 questions (each field
/// names the ranked mechanism it discriminates):
///
/// - `loro_ops`/`loro_changes` climbing while `pages`/`blocks` stay flat →
///   history growth (Finding 6).
/// - `para_cache_entries`/`para_cache_bytes` (A1): the cache is text-keyed, so
///   a keystroke mints an entry; both figures now **sawtooth** rather than
///   climb, because the cache is bounded by retained bytes and rotation drops
///   the superseded versions. A monotonic climb here is the regression. The
///   bytes include the retained Parley layout (the dominant term) and remain a
///   floor only for parley's unreadable capacity slack.
/// - `loro_history_cache` (A2): `true` between the first post-load mutation
///   and the next save (`free_history_cache` runs on the save path only) —
///   an RSS step-down at save while this flips to `false` confirms A2.
/// - `texture_resident`/`texture_peak` (A3): the residency counter's live and
///   peak bytes beside RSS; RSS growth these do **not** show is off-budget
///   (the Vello atlas ratchet is the candidate).
/// - `incremental`: whether the layout came from the incremental path or a
///   full pass. The two visit different fractions of the document, so this is
///   what says whether a full pass (which looks up *every* live paragraph, and
///   so can serve as a cache epoch) is the steady state while typing.
///
/// Throttled to one log per 64 mutations to stay cheap.
fn log_memory_counters(
    loro_doc: &loro::LoroDoc,
    fonts: &loki_layout::SharedFontResources,
    page_count: usize,
    block_count: usize,
    incremental: bool,
) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static MUTATIONS: AtomicU64 = AtomicU64::new(0);
    let n = MUTATIONS.fetch_add(1, Ordering::Relaxed);
    if !n.is_multiple_of(64) {
        return;
    }
    if !tracing::enabled!(target: "loki_text::mem", tracing::Level::INFO) {
        return; // the cache walk below is not free — skip it when nobody listens
    }
    let (para_cache_entries, para_cache_bytes) = fonts.lock().para_cache_stats();
    let textures = appthere_canvas::TextureResidency::snapshot();
    tracing::info!(
        target: "loki_text::mem",
        mutations = n,
        loro_ops = loro_doc.len_ops(),
        loro_changes = loro_doc.len_changes(),
        loro_history_cache = loro_doc.has_history_cache(),
        para_cache_entries,
        para_cache_bytes,
        texture_resident = textures.resident,
        texture_peak = textures.peak,
        pages = page_count,
        blocks = block_count,
        incremental,
        "edit-session memory counters",
    );
}
