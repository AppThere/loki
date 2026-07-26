// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Spec 08 I-23** — is the editor's incremental relayout O(changed) or
//! O(document)?
//!
//! # Why edit position and not document size
//!
//! Wall time against document size cannot separate them: inserting at the *start*
//! legitimately shifts everything after it, so O(N) there is correct behaviour.
//! Sweeping **edit position** discriminates — an edit at the last block should be
//! bounded by the work after it, which is nearly nothing. If both ends are O(N),
//! reuse is not delivering.
//!
//! # What it found (2026-07-26, release profile, best of three)
//!
//! | blocks | at start | at end | no-op | full layout |
//! | ---: | ---: | ---: | ---: | ---: |
//! | 100 | 0.448 ms | 0.415 | 0.002 | 0.405 |
//! | 500 | 2.807 | 2.258 | 0.017 | 2.208 |
//! | 2500 | 16.601 | 11.895 | 0.084 | 11.338 |
//!
//! And the crossover sweep, which is the cleaner statement: **parity at every size
//! from 50 to 2500 blocks.** The incremental path costs the same as a full
//! relayout, which is exactly what "resume from block 0" predicts — it *is* a full
//! relayout, plus bookkeeping.
//!
//! An edit near the **start** is worse still (47% at 2500 blocks): the resync
//! machinery runs and never succeeds, so that overhead is pure loss.
//!
//! **There is no crossover and no size at which reuse pays.** Do not read this as
//! an argument against reuse — it is a measurement of reuse resuming from block 0.
//! Fix the checkpoint density and re-measure before concluding anything about the
//! design.
//!
//! The no-op column is the control that rules out the cheap explanations: an
//! identical document takes the reuse-verbatim early return and costs 0.080 ms at
//! 2500 blocks, so the O(N) term is neither result assembly nor the structural
//! section diff — it is genuine layout work being redone.
//!
//! # Predicted post-fix figures, recorded before implementing (L9-013)
//!
//! The fix under consideration is one checkpoint **per page** — the density both
//! R30 and Spec 09's S9-4/S9-5 need, and the density Q4's claim ("recovering a
//! page costs one page of flow") already assumed.
//!
//! **The same defect breaks both ends of the reflow.** Checkpoints give the
//! *resume* point; resync gives the *stop* point, and resync matches on
//! `cp.block_index == b && cp.checkpoint == s`. With one checkpoint it has exactly
//! one candidate, so early termination can no more fire than resumption can. That
//! is why the mid-document row exists: an edit at the last block exercises only the
//! resume half, since after the last page there is nothing left to stop for.
//!
//! | measurement (2500 blocks) | now | predicted after |
//! | --- | ---: | ---: |
//! | edit at **first** block | 24.143 ms / **327 pages** | ~11 ms / **~327 pages** — unchanged, and correct |
//! | edit at **middle** block | 14.403 ms / **327 pages** | 0.1–0.3 ms / **1–3 pages** |
//! | edit at **last** block | 11.372 ms / **327 pages** | 0.1–0.2 ms / **1–2 pages** |
//! | no-op | 0.097 ms / 0 pages | unchanged |
//! | checkpoints | 1 | **327** (= page count) |
//!
//! **Report pages re-flowed beside elapsed time, and read the pair.** Time alone
//! cannot judge a reuse result: a mid-document edit at 5 ms is either resync
//! failing to stop, or resync working correctly on a document whose pages are
//! packed tightly enough that the pagination cascade legitimately runs a long way.
//! Opposite verdicts, identical stopwatches. Two pages at 5 ms is a per-page cost
//! problem; 150 pages at 5 ms is the mechanism working on a hard document.
//!
//! **The pre-fix counts settle the current case beyond argument: 327 of 327 pages
//! re-flowed at *every* edit position.** The whole document, every time. So the
//! timing spread across positions (24.1 / 14.4 / 11.4) is not layout work at all —
//! the page work is identical — it is the cost of resync attempts that never
//! succeed, which scales with how many blocks `blocks_equal_from` must compare.
//!
//! **Fixture caveat (L9-018).** These are uniform synthetic paragraphs, which may
//! absorb pagination slack more or less readily than real prose. The cascade-depth
//! half of the post-fix prediction (1–3 pages) is therefore the softer half; the
//! checkpoint count is not, being deterministic.
//!
//! **The middle row is the discriminating one, and it separates two failures the
//! other rows cannot:**
//!
//! - **~0.1–0.3 ms** — both halves work.
//! - **~5–6 ms** (about half a relayout) — resume works, **resync does not**. The
//!   reflow starts near the edit and then runs to the end of the document because
//!   nothing tells it to stop.
//! - **~10 ms or more** — neither half took. Check the checkpoint count first: it
//!   is deterministic and settles in one number whether the fix landed at all.
//!
//! Note the current middle figure is *worse than a full relayout* (13.275 against
//! 10.551), not the half-relayout a naive reading suggests — because today it
//! resumes at block 0, never stops early, and pays the failed resync attempts on
//! top. That is the shape to expect from a mechanism that is entered but inert.
//!
//! Edit-at-start is deliberately predicted **not** to improve. A prediction that
//! everything gets faster is unfalsifiable, and position is swept precisely because
//! one end should not move.
//!
//! # Store a line index, not a y-offset
//!
//! The intra-block resume position must go into `PageStart`, and the obvious
//! encoding — the paragraph-local `f32` y at which the page begins — puts a float
//! into a value that resync compares for **equality**. That is the class
//! `LAYOUT_EPSILON_PT` was just added for, and a derived `PartialEq` would be
//! quietly wrong the moment the stored offset is compared against a recomputed one.
//!
//! It is avoidable: `split_and_place_loop` already works in line indices —
//! `split_k` is an index into `para_layout.line_boundaries`, and the split y is
//! simply `line_boundaries[split_k].1`. **Every fragment boundary is a line
//! boundary**, so a `usize` line index carries exactly the same information with
//! exact integer equality, no epsilon and no drift, and lets the derived
//! `PartialEq` stay honest.
//!
//! # Two measurement faults this bench shipped before it was right
//!
//! Recorded because both are the kind that produce a *confident wrong number*
//! rather than an obvious failure.
//!
//! **Cold-cache comparison.** The full-layout column originally built a fresh
//! `FontResources`, so it paid first-touch font loading that the incremental
//! column had already paid. That inflated the comparison baseline and made reuse
//! appear to save 80% at 100 blocks and 50% at 500 — reported as a finding before
//! it was caught. Reuse saves nothing. L08-022's warm-up control exists for
//! precisely this and this bench still walked into it.
//!
//! **Single-shot timing.** The two tables here measure the same quantity and
//! disagreed by ~40% while both were single-shot, which is how the error above
//! surfaced. The difference between these two implementations is smaller than the
//! run-to-run spread, so both now take the best of three. A bench that contradicts
//! itself is worse than one that says nothing.
//!
//! # Root cause, measured 2026-07-26
//!
//! The r22 amendment predicted checkpoint sparsity from the start≈end signature,
//! and the diagnostic confirms it — more sharply than the hypothesis:
//!
//! | blocks | words/block | pages | checkpoints | resume block for an edit at 2499 |
//! | ---: | ---: | ---: | ---: | ---: |
//! | 2500 | 50 | 327 | **1** | **0** |
//! | 2500 | 5 | 55 | **1** | **0** |
//!
//! **327 pages, one checkpoint.** `PaginatedReuse::checkpoints` holds a single
//! entry — the one at block 0 — so `relayout_paginated_incremental`'s resume rule
//! (`checkpoints.rfind(|cp| cp.block_index <= first_changed)`) resolves to block 0
//! for *every* edit, at any position. The incremental path then re-flows the whole
//! document and pays the reuse machinery's overhead on top, which is exactly why
//! it costs more than the full pass it replaces.
//!
//! It is not paragraph length: 5-word blocks produce one checkpoint too.
//!
//! **The mechanism.** `flow_run` records a `PageStart` when, at the *top of a
//! block iteration*, `cursor_y == 0.0 && current_items.is_empty()`. `flush_page`
//! does set both exactly, so the float comparison is not at fault — but the flush
//! happens *inside* a block iteration, when a block overflows the page. By the top
//! of the next iteration the overflowing block's remainder is already placed, so
//! the cursor has moved and the condition is false. A checkpoint is therefore
//! recorded only when a page break falls exactly on a block boundary, which
//! continuous prose almost never does.
//!
//! This is Spec 09 Q4's "checkpoints land only at clean page tops on a block
//! boundary" — but the practical consequence is stronger than that phrasing
//! suggests: for ordinary documents the set is effectively empty, so the whole
//! incremental path is a full relayout wearing a reuse jacket.
//!
//! **Not established:** whether recording a checkpoint after a flush is safe for
//! resync and page numbering. That is a layout-engine change with real blast
//! radius, not a bench's business.
//!
//! # A trap this bench walked into first
//!
//! `LayoutOptions::default()` has `preserve_for_editing: false`, which is the
//! incremental path's very first guard. A probe using the default measures the
//! full-layout fallback while reporting it as the fast path — the first run of
//! this bench did exactly that and read `incr taken = false` everywhere. The
//! options here mirror `loki_text::editing::relayout::edit_opts`.

use std::time::Instant;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_layout::{
    FontResources, LayoutOptions, PaginatedReuse, layout_paginated_full,
    relayout_paginated_incremental,
};

#[path = "support/mod.rs"]
mod support;

fn resources() -> FontResources {
    let mut r = FontResources::new();
    for p in ["/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn doc_of_blocks(n: usize) -> Document {
    doc_of_blocks_sized(n, 50)
}

/// `words_per_block` controls checkpoint density indirectly: a `PageStart` lands
/// only at a clean page top that is also a block boundary, so long paragraphs
/// straddle page tops and produce few checkpoints, while short ones produce many.
/// That is the variable the R30 sparsity hypothesis turns on.
fn doc_of_blocks_sized(n: usize, words_per_block: usize) -> Document {
    let mut doc = Document::new();
    let text = (0..words_per_block)
        .map(|i| format!("word{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    doc.sections[0].blocks = (0..n)
        .map(|_| Block::Para(vec![Inline::Str(text.clone())]))
        .collect();
    doc
}

/// Which checkpoint the incremental path would resume from for an edit at
/// `changed_block`, by the same rule `relayout_paginated_incremental` applies:
/// the last checkpoint in the section at or before the first changed block.
///
/// This is the R30 diagnostic. If it reads near zero for an edit near the end of
/// the document, the resume point is far behind the edit and reuse cannot bound
/// the work however well the rest of the machinery behaves.
fn resume_block_for(reuse: &PaginatedReuse, changed_block: usize) -> Option<usize> {
    reuse
        .checkpoints
        .iter()
        .rfind(|cp| cp.section_index == 0 && cp.block_index <= changed_block)
        .map(|cp| cp.block_index)
}

/// Edits one block by index, returning the mutated document.
fn edit_block(doc: &Document, index: usize) -> Document {
    let mut next = doc.clone();
    if let Some(Block::Para(inlines)) = next.sections[0].blocks.get_mut(index)
        && let Some(Inline::Str(s)) = inlines.first_mut()
    {
        let mut owned = s.to_string();
        owned.push('x');
        *s = owned;
    }
    next
}

fn main() {
    // The editor's own options — `preserve_for_editing` is the incremental path's
    // first guard, and `LayoutOptions::default()` has it false, so a probe using
    // the default measures the fallback and calls it the fast path.
    let opts = LayoutOptions {
        preserve_for_editing: true,
        ..Default::default()
    };
    println!("\nincremental relayout, one character inserted, by edit position");
    println!(
        "  blocks    at start     at mid      at end     no-op   full layout   \
         reflowed pages (start/mid/end)"
    );
    for blocks in [100_usize, 500, 2500] {
        let mut fr = resources();
        let doc = doc_of_blocks(blocks);
        let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);

        // Warm-up and best-of-N come from the shared harness rather than from
        // this file remembering them (L08-035).
        // Reports pages re-flowed alongside elapsed time. Time alone cannot judge
        // a reuse result: a mid-document edit at 5 ms is either resync failing to
        // stop, or resync working on a document whose pages are packed tightly
        // enough that the pagination cascade legitimately runs a long way. Opposite
        // verdicts, identical stopwatches. The count separates them.
        let mut run = |index: usize| -> (f64, usize) {
            let edited = edit_block(&doc, index);
            let mut pages_reflowed = 0;
            let t = support::timing::timed(|| {
                if let Some((_, r)) = relayout_paginated_incremental(
                    &mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts,
                ) {
                    pages_reflowed = r.reflowed_pages;
                }
            });
            (t.best_ms, pages_reflowed)
        };
        let (start_ms, start_pages) = run(0);
        // The mid-document row is the one that exercises *both* halves of the
        // reflow. Checkpoints give the resume point; resync gives the stop point,
        // and resync matches on `cp.block_index == b && cp.checkpoint == s` — so
        // with one checkpoint it has exactly one candidate and early termination
        // can no more fire than resumption can. An edit at the last block tests
        // only the resume half, because after the last page there is nothing left
        // to stop for.
        let (mid_ms, mid_pages) = run(blocks / 2);
        let (end_ms, end_pages) = run(blocks - 1);

        // Discriminator: an *identical* document takes the early return that
        // clones the previous layout verbatim and does no layout work at all. If
        // this costs about what an end-edit costs, the O(N) term is in assembling
        // the result, not in laying anything out.
        let noop_ms = {
            let t = Instant::now();
            let got =
                relayout_paginated_incremental(&mut fr, &doc, &doc, &layout, &reuse, 1.0, &opts);
            assert!(got.is_some(), "an identical document must reuse verbatim");
            t.elapsed().as_secs_f64() * 1000.0
        };

        // The comparison column must share the *same warm* FontResources as the
        // incremental runs above. An earlier revision built a fresh `resources()`
        // here, so this figure carried first-touch font loading that the
        // incremental column had already paid — inflating it and making reuse look
        // like it saved 50-80% when it saves nothing (L08-022: the control exists
        // for exactly this, and this bench still walked into it).
        let full_ms = {
            let edited = edit_block(&doc, blocks - 1);
            support::timing::timed(|| {
                let _ = layout_paginated_full(&mut fr, &edited, 1.0, &opts);
            })
            .best_ms
        };
        println!(
            "  {blocks:>6}  {start_ms:>9.3}  {mid_ms:>9.3}  {end_ms:>9.3}  {noop_ms:>9.3}  \
             {full_ms:>11.3}   {start_pages}/{mid_pages}/{end_pages} of {} pages",
            layout.pages.len()
        );
    }
    println!();

    // ── R30 diagnostic: where does reuse actually resume from? ──────────────
    println!("checkpoint density and resume point (50-word blocks vs 5-word blocks)");
    println!("  blocks  words/blk   pages  checkpoints  resume@last  end-edit ms");
    for &(blocks, wpb) in &[(2500_usize, 50_usize), (2500, 5)] {
        let mut fr = resources();
        let doc = doc_of_blocks_sized(blocks, wpb);
        let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);
        let last = blocks - 1;
        let edited = edit_block(&doc, last);
        let t = Instant::now();
        let _ = relayout_paginated_incremental(&mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts);
        let end_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "  {blocks:>6}  {wpb:>9}  {:>6}  {:>11}  {:>11?}  {end_ms:>11.3}",
            layout.pages.len(),
            reuse.checkpoints.len(),
            resume_block_for(&reuse, last),
        );
    }
    println!();

    // ── Crossover: where does reuse stop paying? The threshold's value should
    // come from this rather than from a round number. ──────────────────────
    println!("crossover sweep — edit at last block vs full relayout");
    println!("  blocks   incremental   full layout   reuse");
    for blocks in [50_usize, 100, 150, 200, 300, 500, 1000, 2500] {
        let mut fr = resources();
        let doc = doc_of_blocks(blocks);
        let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);
        let edited = edit_block(&doc, blocks - 1);
        // `compare` warms *both* sides before timing *either* — the control whose
        // absence produced the retracted r21 figures (L08-035).
        let (incr, full) = {
            let mut fr_a = resources();
            let mut fr_b = resources();
            support::timing::compare(
                || {
                    let _ = relayout_paginated_incremental(
                        &mut fr_a, &edited, &doc, &layout, &reuse, 1.0, &opts,
                    );
                },
                || {
                    let _ = layout_paginated_full(&mut fr_b, &edited, 1.0, &opts);
                },
            )
        };
        println!(
            "  {blocks:>6}   {:>11.3}   {:>11.3}   {}",
            incr.best_ms,
            full.best_ms,
            support::timing::verdict(incr, full),
        );
    }
    println!();

    // The bench asserts rather than only printing (L08-022): if the incremental
    // path stops being entered at all, every number above becomes a measurement
    // of the fallback and the table would still look plausible.
    let mut fr = resources();
    let doc = doc_of_blocks(500);
    let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);
    let edited = edit_block(&doc, 499);
    assert!(
        relayout_paginated_incremental(&mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts)
            .is_some(),
        "the incremental path is no longer entered — these figures would be \
         measuring the full-layout fallback while reporting it as reuse",
    );
}
