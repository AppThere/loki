// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Leak detection (Spec 06 M4 / §7): the Arc-cycle / retained-document and
//! unbounded-cache culprits.
//!
//! Measures the live heap still held after a document open→edit→close cycle,
//! once and after many cycles. A clean cycle leaves a flat residual (everything
//! frees); a **seeded** leak — retaining each opened document, or a cache that
//! never evicts — makes the residual scale with the cycle count, which
//! [`classify_leak`] flags. Asserting the seeded leaks are caught while the clean
//! cycle is not is the M4 acceptance.
//!
//! Run: `cargo bench -p loki-bench --bench leak_detection`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{LeakVerdict, ResidualStats, classify_leak, residual_after};
use loki_doc_model::{delete_text, document_to_loro, insert_text};
use std::hint::black_box;

/// Cycles measured at the "many" repetition count.
const N: usize = 64;
/// Paragraphs per opened document (a chunky working document).
const CYCLE_PARAS: usize = 200;
/// One-time/noise envelope: growth below this is not a per-cycle leak.
const SLACK: u64 = 2 * 1024 * 1024;

/// A full document lifecycle: open (build + reconstruct via CRDT), edit (one
/// keystroke in and out), close (everything dropped at scope end).
fn open_edit_close() {
    let doc = support::build_doc(CYCLE_PARAS, support::WORDS_PER_PARA);
    let loro = document_to_loro(&doc).expect("document_to_loro");
    let _ = insert_text(&loro, 0, 0, "x");
    let _ = delete_text(&loro, 0, 0, 1);
}

fn report(label: &str, one: ResidualStats, many: ResidualStats, verdict: LeakVerdict) {
    eprintln!(
        "  {label}:\n    after 1: {} B / {} allocs   after {N}: {} B / {} allocs  → {verdict:?}",
        one.curr_bytes, one.curr_blocks, many.curr_bytes, many.curr_blocks,
    );
}

fn main() {
    eprintln!("\nleak detection — residual live heap after open/edit/close cycles:");

    // Warm any one-time globals so both measurements start from the same state.
    open_edit_close();

    // (a) Clean cycle — residual must stay flat as cycles grow.
    let clean_1 = residual_after(1, open_edit_close);
    let clean_n = residual_after(N, open_edit_close);
    let clean = classify_leak(clean_1.curr_bytes, clean_n.curr_bytes, SLACK);
    report("clean open/edit/close", clean_1, clean_n, clean);

    // (b) Seeded leak: retain every opened document (an Arc cycle never frees it).
    let mut retained: Vec<_> = Vec::new();
    let open_retain = |sink: &mut Vec<_>| {
        let doc = support::build_doc(CYCLE_PARAS, support::WORDS_PER_PARA);
        sink.push(document_to_loro(&doc).expect("document_to_loro"));
    };
    let leak_1 = residual_after(1, || open_retain(&mut retained));
    let leak_n = residual_after(N, || open_retain(&mut retained));
    let leak = classify_leak(leak_1.curr_bytes, leak_n.curr_bytes, SLACK);
    report("seeded retained-document leak", leak_1, leak_n, leak);
    black_box(&retained);

    // (c) Seeded unbounded cache: a cache that never evicts (64 KiB per entry).
    let mut cache: Vec<Vec<u8>> = Vec::new();
    let cache_1 = residual_after(1, || cache.push(vec![0u8; 64 * 1024]));
    let cache_n = residual_after(N, || cache.push(vec![0u8; 64 * 1024]));
    let cache_v = classify_leak(cache_1.curr_bytes, cache_n.curr_bytes, SLACK);
    report("seeded unbounded cache", cache_1, cache_n, cache_v);
    black_box(&cache);

    // Acceptance: both seeded leaks caught; the clean cycle returns to baseline.
    assert!(leak.leaks(), "seeded retained-document leak was not caught");
    assert!(cache_v.leaks(), "seeded unbounded cache was not caught");
    assert!(
        !clean.leaks(),
        "clean open/edit/close was flagged as leaking — real retention on the \
         open/edit/close path (investigate before trusting the detector)",
    );
    eprintln!("\nboth seeded leaks caught; clean cycle returned to baseline.");
}
