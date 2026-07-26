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
//! # What it found (2026-07-26, release profile)
//!
//! The incremental path *is* entered, and reuse stops paying as the document
//! grows — then costs more than not reusing at all:
//!
//! | blocks | at start | at end | no-op | full layout | reuse verdict |
//! | ---: | ---: | ---: | ---: | ---: | --- |
//! | 100 | 0.850 ms | 0.421 | 0.002 | 2.110 | saves 80% |
//! | 500 | 4.534 | 2.977 | 0.016 | 5.917 | saves 50% |
//! | 2500 | 18.174 | **16.936** | 0.149 | **12.903** | **31% slower than full** |
//!
//! At 2500 blocks (~500 pages) an edit to the **last block** — where reuse should
//! be near-free — costs 16.9 ms against a 12.9 ms full relayout. That is a dropped
//! frame per keystroke, and the incremental path is the reason rather than the
//! remedy.
//!
//! The no-op column is the discriminator that rules out the cheap explanations:
//! an identical document takes the reuse-verbatim early return and costs 0.149 ms
//! at 2500 blocks, so the O(N) term is neither result assembly nor the structural
//! section diff — it is genuine layout work being redone for content the edit
//! cannot have affected.
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
    println!("  blocks    at start     at end     no-op   full layout   incr taken?");
    for blocks in [100_usize, 500, 2500] {
        let mut fr = resources();
        let doc = doc_of_blocks(blocks);
        let (layout, reuse) = layout_paginated_full(&mut fr, &doc, 1.0, &opts);

        let mut run = |index: usize| -> (f64, bool) {
            let edited = edit_block(&doc, index);
            let t = Instant::now();
            let got =
                relayout_paginated_incremental(&mut fr, &edited, &doc, &layout, &reuse, 1.0, &opts);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            (ms, got.is_some())
        };
        let (start_ms, start_incr) = run(0);
        let (end_ms, end_incr) = run(blocks - 1);

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

        let full_ms = {
            let mut fr2 = resources();
            let edited = edit_block(&doc, blocks - 1);
            let t = Instant::now();
            let _ = layout_paginated_full(&mut fr2, &edited, 1.0, &opts);
            t.elapsed().as_secs_f64() * 1000.0
        };
        println!(
            "  {blocks:>6}  {start_ms:>9.3}  {end_ms:>9.3}  {noop_ms:>9.3}  {full_ms:>11.3}   \
             start={start_incr} end={end_incr}"
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
