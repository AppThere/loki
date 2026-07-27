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
//! bounded by the work after it, which is nearly nothing.
//!
//! **The answer is that position does not matter at all**, because reuse resumes
//! from block 0 in every case — see the checkpoint section below. The apparent
//! position dependence in earlier revisions was this bench's own measurement
//! ordering, retracted below.
//!
//! # The position-dependent spread does not exist — it was this bench's ordering
//!
//! Two attributions were made for an apparent spread across edit positions (start
//! 18.2 ms, mid 15.4, end 12.0 at 2500 blocks): first `blocks_equal_from`
//! comparison cost, then an unidentified O(N^1.8) term. **Both were explaining an
//! artifact.**
//!
//! The comparison counter refuted the first, and *backwards* — comparisons rise as
//! time falls (2,502 / 3,752 / 5,001 against 18.2 / 15.4 / 12.0), and the counts
//! are fully accounted for by prefix + suffix + `blocks_equal_from` to the unit.
//!
//! The second fell to an ordering check. The three positions were timed in sequence
//! against one shared `FontResources`, so the first call paid process- and
//! document-level first touch and each later one inherited it warm — producing a
//! monotonic decrease that reads exactly like "cost falls with edit position".
//! **Reversing the order reverses the sign:**
//!
//! | fixture | start-first / end-second | end-first / start-second |
//! | --- | --- | --- |
//! | 2500 blocks, 50 w/b | 12.311 / 12.057 → +0.254 | 12.863 / 12.226 → **−0.637** |
//! | 2500 blocks, 5 w/b | 7.573 / 7.449 → +0.124 | 7.627 / 7.420 → **−0.207** |
//!
//! Whichever position runs first is slower. With all positions warmed before any is
//! timed, the spread is gone: 11.561 / 14.339 / 12.953 at 2500 blocks — not
//! monotonic, and start is now the cheapest of the three.
//!
//! **So edit position has no measurable effect on incremental relayout cost**, which
//! is exactly what "resume from block 0 and re-flow everything" predicts. The
//! mechanism's behaviour is fully explained by the checkpoint finding, with nothing
//! left over.
//!
//! # This bench cannot resolve small timing differences at all
//!
//! Measured, rather than assumed: running the *same unchanging subject* through the
//! harness five times gives a spread of **4.4% on one run and 36.5% on the next**.
//! The noise floor is not merely large, it is **unstable between runs**.
//!
//! `verdict`'s ±5% band was a chosen threshold with nothing establishing that 5% is
//! resolvable, and it is not. Any difference this bench reports below roughly a
//! third is noise, and reporting it as a percentage invites exactly the confident
//! misreading that produced three retracted attributions in a row.
//!
//! **So read the deterministic columns and distrust the timings.** Pages re-flowed,
//! checkpoint count and block comparisons are exact and reproducible; they are what
//! established every real finding here — 327 pages with one checkpoint, resume at
//! block 0 always, 327/327 pages re-flowed at every position. The timing columns are
//! only load-bearing where the gap is an order of magnitude, which T3.4's predicted
//! 11 ms → 0.1 ms is by a factor of ~40.
//!
//! One casualty of measuring this: the "mid is highest" reading from the previous
//! revision does not survive `sweep` (10.196 / 10.253 / 10.870 at 2500 blocks). It
//! was ordering leakage again, in a third form — the pre-warm removed the monotonic
//! artifact but three sequential `timed` calls still shared state.
//!
//! # The harness gap this exposed
//!
//! `support::timing::compare` warms both sides before timing either, which is the
//! control that caught the cold-cache fault. It does **not** help across *separate*
//! `timed` calls that share mutable state — and three sequential `timed` calls
//! against one `FontResources` is precisely that case. The rule the harness cannot
//! enforce for you: **when several measurements share warm-able state, warm every
//! subject before timing any of them.** This bench now does that explicitly.
//!
//! Two wrong attributions in a row, both of an artifact, is also the argument for
//! reaching for an ordering check *before* a third hypothesis.
//!
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
//! re-flowed at *every* edit position.** The whole document, every time.
//!
//! # The timing spread is *not* comparison cost — an attribution, refuted
//!
//! Page work is constant, so the spread across positions is something else, and it
//! was attributed here to the diff scans. **The comparison counter refutes that,
//! and backwards:**
//!
//! | position | block comparisons | time |
//! | --- | ---: | ---: |
//! | start | 2,502 | 18.405 ms |
//! | mid | 3,752 | 15.435 ms |
//! | end | **5,001** | **11.975 ms** |
//!
//! Comparisons rise as time falls. Edit-at-end performs twice the comparisons in
//! two-thirds the time, so they cannot be what the spread is made of.
//!
//! The counts are fully accounted for, which is what makes this a refutation
//! rather than noise: `common_prefix_len` scans from the front, `common_suffix_len`
//! from the back, and `blocks_equal_from` from `from` to the first difference. For
//! an edit at block 0 that is 1 + 2500 + 1 = 2502; at the last block,
//! 2500 + 1 + 2500 = 5001; at the middle, 1251 + 1251 + 1250 = 3752. All three
//! match to the unit.
//!
//! **What the spread actually is remains unidentified**, and is recorded as unknown
//! rather than guessed at a second time.
//!
//! # Why T3.4's prediction survives anyway — and where it still might not
//!
//! The worry was that unexplained *fixed* overhead would remain once the resume
//! point moves. Cross-size scaling bounds that from data already here. Taking the
//! edit-at-end column, which is the cleanest:
//!
//! | pages | time | per page |
//! | ---: | ---: | ---: |
//! | 14 | 0.375 ms | 0.0268 |
//! | 66 | 2.171 | 0.0329 |
//! | 327 | 10.982 | 0.0336 |
//!
//! A straight line through the endpoints is `t ≈ 0.0339 × pages` with an intercept
//! of **−0.099 ms** — no positive fixed cost — and it predicts the 66-page row to
//! within 1.5%. A large position-independent constant would show up as a much
//! higher per-page figure at the small end, and it does not. So 3 pages lands near
//! **0.10 ms**, inside the predicted band, derived from measured scaling rather than
//! from any theory of what the spread is. **T3.4 does not need the spread explained
//! first.**
//!
//! **The gap that argument left open is now closed, in the prediction's favour.**
//! It bounded a *constant* while leaving room for a position-dependent term that
//! might survive T3.4. That term does not exist (see above), so the cost model is
//! simply `t ≈ 0.034 ms × pages re-flowed` with nothing else in it. The predicted
//! band needs no caveat.
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
    // ── The noise floor, measured before anything is read against it ─────────
    //
    // Every figure below is a difference between timings, and a difference is only
    // a finding if it exceeds what this harness can resolve. `verdict`'s +/-5% was
    // a chosen band with nothing establishing that 5% is resolvable — so measure
    // the same unchanging subject repeatedly and let the spread set the threshold.
    let noise_floor = {
        let opts = LayoutOptions {
            preserve_for_editing: true,
            ..Default::default()
        };
        let mut fr = resources();
        let doc = doc_of_blocks(2500);
        let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);
        let edited = edit_block(&doc, 1250);
        let r = support::timing::repeatability(5, || {
            let _ =
                relayout_paginated_incremental(&mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts);
        });
        let ratio = r.spread_ratio() - 1.0;
        println!(
            "harness noise floor (same subject, 5 rounds of best-of-3): \
             {:.3}–{:.3} ms, spread {:.1}%",
            r.best_ms,
            r.worst_ms,
            ratio * 100.0
        );
        println!(
            "  => differences below {:.1}% are not resolvable by this bench",
            ratio * 100.0
        );
        println!(
            "  NOTE: this floor is itself unstable — 4.4% and 36.5% on consecutive\n\
             \x20       runs of this machine. Treat the DETERMINISTIC columns (pages\n\
             \x20       re-flowed, checkpoints, block comparisons) as this bench's\n\
             \x20       findings, and timings only where the gap is an order of\n\
             \x20       magnitude — as T3.4's 11 ms -> 0.1 ms prediction is.\n"
        );
        ratio
    };

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

        // Pages re-flowed and block comparisons are *counts*, not timings, so they
        // need no warm-up discipline — they are deterministic for a given edit.
        let mut counts_at = |index: usize| -> (usize, u64) {
            let edited = edit_block(&doc, index);
            loki_layout::reset_block_comparisons();
            let pages =
                relayout_paginated_incremental(&mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts)
                    .map_or(0, |(_, r)| r.reflowed_pages);
            (pages, loki_layout::block_comparisons())
        };
        let (start_pages, start_cmp) = counts_at(0);
        let (mid_pages, mid_cmp) = counts_at(blocks / 2);
        let (end_pages, end_cmp) = counts_at(blocks - 1);

        // All three positions are timed by `sweep`, which warms every subject
        // before timing any of them. Three sequential `timed` calls cannot get that
        // control, and that is exactly what produced the retracted "cost falls with
        // edit position" artifact — L08-035 made enforceable rather than remembered.
        let edited_start = edit_block(&doc, 0);
        let edited_mid = edit_block(&doc, blocks / 2);
        let edited_end = edit_block(&doc, blocks - 1);
        let timings = {
            let mut fr_a = resources();
            let mut fr_b = resources();
            let mut fr_c = resources();
            let mut a = || {
                let _ = relayout_paginated_incremental(
                    &mut fr_a,
                    &edited_start,
                    &doc,
                    &layout,
                    &reuse,
                    1.0,
                    &opts,
                );
            };
            let mut b = || {
                let _ = relayout_paginated_incremental(
                    &mut fr_b,
                    &edited_mid,
                    &doc,
                    &layout,
                    &reuse,
                    1.0,
                    &opts,
                );
            };
            let mut c = || {
                let _ = relayout_paginated_incremental(
                    &mut fr_c,
                    &edited_end,
                    &doc,
                    &layout,
                    &reuse,
                    1.0,
                    &opts,
                );
            };
            support::timing::sweep(&mut [&mut a, &mut b, &mut c])
        };
        let start_ms = timings[0].best_ms;
        let mid_ms = timings[1].best_ms;
        let end_ms = timings[2].best_ms;

        // The mid-document row is the one that exercises *both* halves of the
        // reflow. Checkpoints give the resume point; resync gives the stop point,
        // and resync matches on `cp.block_index == b && cp.checkpoint == s` — so
        // with one checkpoint it has exactly one candidate and early termination
        // can no more fire than resumption can. An edit at the last block tests
        // only the resume half, because after the last page there is nothing left
        // to stop for.

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
             {full_ms:>11.3}   {start_pages}/{mid_pages}/{end_pages} of {} pages   \
             cmp {start_cmp}/{mid_cmp}/{end_cmp}",
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

    // ── Does the position-dependent spread scale with pages or with blocks? ──
    //
    // The two are confounded in every row above, because block count and page count
    // move together there. Holding *blocks* fixed at 2500 and varying words-per-
    // block separates them at 6x: 50 words gives 327 pages, 5 words gives 55, and
    // both still re-flow the whole document. If the spread is per-page the short row
    // shows roughly a sixth of it; if per-block it is unchanged.
    //
    // Worth settling before T3.4 rather than after, for a narrow reason: if the
    // component scales with blocks, T3.4's 0.1-0.3 ms band is wrong, mid and end
    // edits land high, and the natural reading is that resume did not take — when
    // the checkpoint count would say it did. Better to enter with a prediction that
    // can be trusted than to spend the post-fix session disentangling two failures.
    println!("spread basis — 2500 blocks throughout, words/block varied");
    println!("  words/blk   pages   at start     at end     spread   per page   per block");
    for wpb in [50_usize, 5] {
        let mut fr = resources();
        let doc = doc_of_blocks_sized(2500, wpb);
        let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);
        let mut timed_at = |index: usize| {
            let edited = edit_block(&doc, index);
            support::timing::timed(|| {
                let _ = relayout_paginated_incremental(
                    &mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts,
                );
            })
            .best_ms
        };
        // Measured in both orders. The main table runs start, then mid, then end
        // against one shared FontResources, so each later position begins with a
        // warmer paragraph cache — which would produce a monotonic decrease with no
        // dependence on edit position at all. Reversing the order is what tells the
        // two apart, and it is the ordering control L08-022 asks for applied
        // *across* measurements rather than within one.
        let start_first = timed_at(0);
        let end_second = timed_at(2499);
        let end_first = timed_at(2499);
        let start_second = timed_at(0);
        let (start, end) = (start_first, end_second);
        let spread = start - end;
        let pages = layout.pages.len();
        println!(
            "  {wpb:>9}   {pages:>5}   {start:>8.3}   {end:>8.3}   {spread:>8.3}   \
             {:>8.5}   {:>9.6}",
            spread / pages as f64,
            spread / 2500.0,
        );
        println!(
            "            reversed order: end-first {end_first:.3}, start-second \
             {start_second:.3} (spread {:.3})",
            start_second - end_first,
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
            support::timing::verdict_within(incr, full, noise_floor.max(0.05)),
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
