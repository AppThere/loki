// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Spec 09 S9-3 §10i** — is the per-keystroke page scan free today?
//!
//! `page_locate::recompute_page_index` walks pages from index 0 until it finds
//! the one holding the caret, dereferencing each page's `editing_data` and
//! scanning its paragraph list on the way. It runs on **every keystroke**
//! (`editor_keydown_text.rs`, `editor_keydown.rs`, `editor_keydown_ctrl.rs`).
//!
//! Its access set is `0..=M` for a caret on page `M` — from the code's shape;
//! this bench does *not* establish it, see below — which is why S9-3 exists:
//! under windowing every page in that set must be resident, so the scan would
//! defeat the windowing it is meant to enable (R9-04).
//!
//! But the access set is a residency argument, and this bench asks a different
//! question that decides how S9-3 should be *scoped*: **is the walk measurable
//! work today, with every page already resident?** If typing on page 300 costs
//! materially more than typing on page 1, this is a present-day typing-latency
//! defect on long documents — a user-visible fix adjacent to Spec 08's I-05 —
//! rather than groundwork for eviction. If it is noise, S9-3 stays architecture.
//!
//! Wall-clock, not allocations: nothing is allocated here, and the cost is
//! pointer-chasing over page and paragraph vectors.
//!
//! # What this bench does and does not show
//!
//! It shows **cost**: ~3.3 us at 445 pages, ~13.6 us at 889, flat in the caret's
//! page, with a guaranteed miss costing about the same as a hit. Against a ~16 ms
//! frame that settles the question it was built for — the scan is not a
//! present-day latency defect.
//!
//! It does **not** show control flow. The flat curve was once read as proof that
//! the `visible` early exit never fires (Spec 09 R9-18); a characterisation test
//! on real geometry then showed it does fire, and R9-18 was retracted. The
//! timing remains unexplained. A clock measures time; a claim about which branch
//! runs needs its own observation, which is the counting accessor S9-3 has to
//! build anyway.
//!
//! Run: `cargo bench -p loki-text --bench page_locate_latency`

use std::hint::black_box;
use std::time::Instant;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_text::editing::cursor::DocumentPosition;
use loki_text::editing::page_locate::recompute_page_index;

/// Paragraphs in the test document. Sized to produce several hundred pages so
/// the near/far comparison spans a realistic long document rather than a toy.
const PARAS: &[usize] = &[4_000, 8_000];

/// Timed repetitions per position. The work is microseconds at most, so a single
/// call is below timer resolution.
const REPS: usize = 2_000;

fn build_doc(paras: usize) -> Document {
    let words = [
        "document",
        "layout",
        "paragraph",
        "cursor",
        "render",
        "office",
        "shaping",
        "baseline",
        "indent",
        "column",
        "measure",
        "typeset",
    ];
    let blocks: Vec<Block> = (0..paras)
        .map(|i| {
            let mut s = format!("{}. ", i + 1);
            for j in 0..40 {
                s.push_str(words[(i + j) % words.len()]);
                s.push(' ');
            }
            Block::Para(vec![Inline::Str(s)])
        })
        .collect();
    let section = Section::with_layout_and_blocks(PageLayout::default(), blocks);
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// The block index of the first paragraph laid out on `page`, so the timed
/// position is one the scan can actually find.
fn first_block_on_page(layout: &loki_layout::PaginatedLayout, page: usize) -> Option<usize> {
    layout
        .pages
        .get(page)?
        .editing_data
        .as_ref()?
        .paragraphs
        .first()
        .map(|p| p.block_index)
}

/// Median of `REPS` timed calls, in nanoseconds. Median rather than mean: a
/// scheduler hiccup in one repetition should not decide the comparison.
fn time_at(layout: &loki_layout::PaginatedLayout, block: usize) -> (u128, usize) {
    // `page_index` is deliberately **stale** (0), not the answer. Seeding it with
    // the correct page makes `new_page == pos.page_index` trivially true, so the
    // function returns the same value whether it found the paragraph or scanned
    // the whole document and found nothing — the timing would then be a number
    // with no established meaning. A stale index is also the realistic case: the
    // caret moved and the page has to be *re*-computed, which is why the
    // function is called at all.
    let pos = DocumentPosition {
        page_index: 0,
        paragraph_index: block,
        byte_offset: 0,
        path: Vec::new(),
    };
    // Sentinel: the scan must actually locate the paragraph on `page`. If it
    // does not, every timing below measures a failed lookup rather than the
    // work being characterised (R9-13).
    let found = recompute_page_index(layout, &pos).page_index;
    // Warm the caches this position touches, outside the timed region (L9-011).
    for _ in 0..64 {
        black_box(recompute_page_index(layout, &pos));
    }
    let mut samples = Vec::with_capacity(REPS);
    for _ in 0..REPS {
        let t = Instant::now();
        black_box(recompute_page_index(layout, &pos));
        samples.push(t.elapsed().as_nanos());
    }
    samples.sort_unstable();
    (samples[samples.len() / 2], found)
}

fn run(paras: usize) -> (usize, u128, u128, u128) {
    let doc = build_doc(paras);
    let mut resources = FontResources::new();
    let layout = layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
            ..Default::default()
        },
    );
    let DocumentLayout::Paginated(layout) = layout else {
        eprintln!("paginated mode did not return a paginated layout");
        return (0, 0, 0, 0);
    };

    // Timer-resolution floor. Every median below is meaningless if the clock
    // cannot resolve smaller than them, and near-identical medians across very
    // different workloads is exactly what a coarse clock looks like (R9-13).
    {
        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let t = Instant::now();
            black_box(0u64);
            samples.push(t.elapsed().as_nanos());
        }
        samples.sort_unstable();
        eprintln!(
            "  timer floor (empty region): median {:>9} ns  min {:>9} ns",
            samples[samples.len() / 2],
            samples[0]
        );
    }

    let pages = layout.pages.len();
    eprintln!("\n  ── {pages} pages, {paras} paragraphs ──────────────────────────");

    // Sample across the document rather than only the ends: two points cannot
    // distinguish "linear in M" from "a constant step somewhere".
    let probes: Vec<usize> = [0usize, 1, pages / 8, pages / 4, pages / 2, pages - 1]
        .into_iter()
        .filter(|p| *p < pages)
        .collect();

    let mut first: Option<u128> = None;
    let mut last = 0u128;
    let full_scan;
    for page in probes {
        let Some(block) = first_block_on_page(&layout, page) else {
            eprintln!("  page {page:>4}: no editing data — skipped");
            continue;
        };
        let (ns, found) = time_at(&layout, block);
        assert_eq!(
            found, page,
            "recompute_page_index resolved block {block} to page {found}, not {page} — \
             the probe is measuring a lookup that does not find its target"
        );
        first.get_or_insert(ns);
        last = ns;
        eprintln!(
            "  page {page:>4}  median {ns:>9} ns  ({:.3} ms)  resolved->{found}",
            ns as f64 / 1e6
        );
    }

    // Decisive control: a block index that exists nowhere forces the loop to
    // run to completion over every page with no early break and no cursor_rect.
    // That is the scan and nothing else. If it costs about what a page-0 hit
    // costs, the walk is genuinely cheap and the flat curve above is real; if it
    // costs far more, the hit probes are not scanning as far as the code shape
    // suggests and the flatness is an artefact.
    {
        let pos = DocumentPosition {
            page_index: 0,
            paragraph_index: paras + 10_000,
            byte_offset: 0,
            path: Vec::new(),
        };
        for _ in 0..64 {
            black_box(recompute_page_index(&layout, &pos));
        }
        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let t = Instant::now();
            black_box(recompute_page_index(&layout, &pos));
            samples.push(t.elapsed().as_nanos());
        }
        samples.sort_unstable();
        full_scan = samples[samples.len() / 2];
        eprintln!(
            "  full scan (block not present, all {pages} pages, no cursor_rect): \
             median {full_scan:>9} ns"
        );
    }

    (pages, first.unwrap_or(0), last, full_scan)
}

fn main() {
    eprintln!("Spec 09 S9-3 — recompute_page_index cost by caret page");
    eprintln!(
        "  Runs on every keystroke. §10i modelled its access set as pages 0..=M, so\n           cost should grow with the caret's page. Two document sizes test that: if the\n           cost is flat in M but doubles with total page count, the loop is running to\n           completion every time and the access set is the WHOLE document, not a prefix."
    );

    let mut prior: Option<(usize, u128)> = None;
    for &paras in PARAS {
        let (pages, first, last, full) = run(paras);
        if pages == 0 {
            continue;
        }
        eprintln!(
            "  flat-in-M ratio (last page / first page): {:.2}×",
            last as f64 / first.max(1) as f64
        );
        if let Some((prev_pages, prev_full)) = prior {
            eprintln!(
                "  scales-with-N ratio vs previous size: pages {:.2}×, full scan {:.2}×",
                pages as f64 / prev_pages as f64,
                full as f64 / prev_full.max(1) as f64,
            );
        }
        prior = Some((pages, full));
    }
    eprintln!(
        "\n  A keystroke has ~16 ms before it costs a frame. Read the absolute numbers\n           against that, not against each other."
    );
}
