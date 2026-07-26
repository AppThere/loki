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
//! **Not established:** why prefix reuse fails to bound it. `ParaCache` hit/miss
//! counters would localise it and do not exist yet.
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
    FontResources, LayoutOptions, layout_paginated_full, relayout_paginated_incremental,
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
    let mut doc = Document::new();
    let text = (0..50)
        .map(|i| format!("word{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    doc.sections[0].blocks = (0..n)
        .map(|_| Block::Para(vec![Inline::Str(text.clone())]))
        .collect();
    doc
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
