// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loro history growth (Spec 06 M4 / §7): the sneakiest culprit — it grows with
//! session *time*, not document size.
//!
//! Simulates a long editing session (insert then delete a character, so the
//! document length is stable) and **measures and reports** how the CRDT oplog and
//! tombstones grow: the op/change counters (`len_ops` / `len_changes`, the same
//! signals `loki_text::mem` logs) and the live heap the session retains. This
//! confirms and quantifies the known unbounded growth (memory-audit Finding 6 /
//! `TODO(loro-compaction)` — now addressed by `loro_bridge::compact`).
//!
//! A second phase runs the same workload with `compact_history` applied every
//! `COMPACT_EVERY` keystrokes (the editor applies it after saves) and asserts
//! the curve flattens: the compacted session's final oplog must be a small
//! fraction of the uncompacted one.
//!
//! Run: `cargo bench -p loki-bench --bench leak_loro_history`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::residual_after;
use loki_doc_model::loro_bridge::compact_history;
use loki_doc_model::{delete_text, document_to_loro, insert_text};
use std::hint::black_box;

const KEYSTROKES: usize = 5_000;
const COMPACT_EVERY: usize = 1_000;

fn main() {
    let doc = support::build_doc(20, support::WORDS_PER_PARA);
    let loro = document_to_loro(&doc).expect("document_to_loro");
    let ops0 = loro.len_ops();
    let changes0 = loro.len_changes();

    let residual = residual_after(1, || {
        for _ in 0..KEYSTROKES {
            let _ = insert_text(&loro, 0, 0, "x");
            let _ = delete_text(&loro, 0, 0, 1);
        }
    });

    let ops1 = loro.len_ops();
    let changes1 = loro.len_changes();
    let d_ops = ops1 - ops0;
    let d_changes = changes1 - changes0;

    eprintln!(
        "\nloro history over {KEYSTROKES} keystrokes (insert+delete; length stable):\n  \
         ops:     {ops0} → {ops1}  (+{d_ops})\n  \
         changes: {changes0} → {changes1}  (+{d_changes})\n  \
         live heap retained by the session: {} B / {} allocs\n  \
         per-keystroke: {:.2} ops, {} B",
        residual.curr_bytes,
        residual.curr_blocks,
        d_ops as f64 / KEYSTROKES as f64,
        residual.curr_bytes / KEYSTROKES as u64,
    );
    eprintln!(
        "  → history grows with edit count (Finding 6); the compacted phase \
         below must flatten this."
    );

    // Measured & reported (not a pass/fail): editing must move the oplog.
    assert!(ops1 > ops0, "oplog did not grow — did the edits apply?");
    black_box(&loro);

    // ── Phase 2: same workload, compacting every COMPACT_EVERY keystrokes ──
    let doc2 = support::build_doc(20, support::WORDS_PER_PARA);
    let mut compacted = document_to_loro(&doc2).expect("document_to_loro");
    for i in 0..KEYSTROKES {
        let _ = insert_text(&compacted, 0, 0, "x");
        let _ = delete_text(&compacted, 0, 0, 1);
        if (i + 1) % COMPACT_EVERY == 0 {
            compacted = compact_history(&compacted).expect("compact_history");
        }
    }
    let ops_compacted = compacted.len_ops();
    eprintln!(
        "with compact_history every {COMPACT_EVERY} keystrokes:\n  \
         ops after {KEYSTROKES} keystrokes: {ops_compacted} (uncompacted: {ops1})"
    );

    // The whole point of the fix: history no longer grows with session time.
    // Bound = one compaction window's worth of ops plus the baseline.
    assert!(
        ops_compacted < ops0 + 3 * COMPACT_EVERY,
        "compacted oplog did not flatten: {ops_compacted} ops"
    );
    black_box(&compacted);
}
