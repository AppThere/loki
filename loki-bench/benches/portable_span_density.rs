// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! P-3 measurement bench (audit `docs/audit-2026-06.md` P-3, deferred-features
//! plan 6.5): are the per-glyph-run span lookups in `loki-layout`'s paragraph
//! emission (`span_scale_for_range` / `span_covering_range` / the per-glyph
//! scaling probe) a real bottleneck?
//!
//! Each is a linear `spans.iter().find(..)` re-run per glyph run, so emission is
//! O(runs × spans) — quadratic when a paragraph has many style spans. The audit
//! flagged it "P2, re-measure first". This bench measures two things so the fix
//! decision is data-driven, not speculative:
//!
//! 1. **End-to-end**: wall-clock to lay out one fixed-length paragraph split
//!    into `N` distinct style spans, sweeping `N`. If total layout stays ~linear
//!    in `N`, the O(N²) scans are a negligible fraction in practice (Parley
//!    shaping dominates) and the fix is unjustified. Super-linear growth
//!    attributable to the scans would confirm it.
//! 2. **Isolated**: the raw `find`-per-run scan pattern over `N` synthetic spans
//!    doing `N` lookups (the exact predicate emission uses), linear vs a
//!    byte→span-index prefix map, to quantify the absolute scan cost per
//!    paragraph.
//!
//! Wall-clock, so this is the **device axis** (noisy locally); we report
//! per-op medians and the N→N scaling ratio, which survives noise.
//!
//! Run: `cargo bench -p loki-bench --bench portable_span_density`

use std::hint::black_box;
use std::time::Instant;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_layout::{FontResources, LayoutOptions, layout_paginated_full};
use loki_primitives::color::{DocumentColor, RgbColor};

/// Total words in each test paragraph, held constant across `N` so only the
/// span *count* varies (more spans = more, shorter runs over the same text).
const WORDS: usize = 256;
const WORD_POOL: &[&str] = &[
    "loki",
    "document",
    "layout",
    "reflow",
    "paragraph",
    "glyph",
    "render",
    "shaping",
];
/// Span counts to sweep.
const SPAN_COUNTS: &[usize] = &[4, 16, 64, 256];

/// Build a paragraph of `WORDS` words split into `n` runs, each carrying a
/// distinct RGB colour so it flattens to a distinct `StyleSpan`.
fn paragraph_with_spans(n: usize) -> Block {
    let per_run = (WORDS / n).max(1);
    let mut content = Vec::with_capacity(n);
    for run in 0..n {
        let mut text = String::new();
        for w in 0..per_run {
            if w > 0 {
                text.push(' ');
            }
            text.push_str(WORD_POOL[(run + w) % WORD_POOL.len()]);
        }
        text.push(' ');
        // Distinct colour per run → distinct span.
        let shade = (run % 255) as f32 / 255.0;
        let props = CharProps {
            color: Some(DocumentColor::Rgb(RgbColor::new(shade, 1.0 - shade, 0.5))),
            ..Default::default()
        };
        content.push(Inline::StyledRun(StyledRun {
            style_id: None,
            direct_props: Some(Box::new(props)),
            content: vec![Inline::Str(text)],
            attr: Default::default(),
        }));
    }
    Block::Para(content)
}

fn doc_with(n: usize) -> Document {
    let section =
        Section::with_layout_and_blocks(PageLayout::default(), vec![paragraph_with_spans(n)]);
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// Median of `samples` runs of `f`, each doing `inner` repetitions (cache
/// cleared before each rep so the full emit path — where the scans live —
/// runs cold, as it does for the edited paragraph on a keystroke).
fn median_us(samples: usize, inner: usize, mut f: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = Vec::with_capacity(samples);
    for _ in 0..samples {
        let t = Instant::now();
        for _ in 0..inner {
            f();
        }
        times.push(t.elapsed().as_secs_f64() * 1e6 / inner as f64);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

fn bench_end_to_end() {
    eprintln!("── P-3 end-to-end: layout one {WORDS}-word paragraph vs span count ──");
    eprintln!("   (paragraph cache cleared each rep → cold emit path)");
    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };
    // Warm the font stack once, outside the measured region.
    let warm = doc_with(4);
    let _ = layout_paginated_full(&mut resources, &warm, 1.0, &options);

    let mut prev: Option<(usize, f64)> = None;
    for &n in SPAN_COUNTS {
        let doc = doc_with(n);
        let us = median_us(9, 20, || {
            resources.clear_paragraph_cache();
            let (layout, _) = layout_paginated_full(&mut resources, &doc, 1.0, &options);
            black_box(&layout);
        });
        let ratio = prev.map(|(pn, pt)| format!("{:.2}× for {}× spans", us / pt, n / pn));
        eprintln!(
            "   spans={n:<4} layout={us:>9.2} µs/para   {}",
            ratio.unwrap_or_default()
        );
        prev = Some((n, us));
    }
}

fn bench_isolated_scan() {
    eprintln!("── P-3 isolated: N lookups over N spans, linear find vs indexed ──");
    // Reproduces the exact emission predicate: for each of N runs, find the span
    // whose range contains the run's range. Linear = current; indexed = the
    // proposed byte→span-index prefix map.
    for &n in SPAN_COUNTS {
        // N spans tiling [0, N); N run-ranges, one per span.
        let spans: Vec<std::ops::Range<usize>> = (0..n).map(|i| i..i + 1).collect();
        let runs: Vec<std::ops::Range<usize>> = (0..n).map(|i| i..i + 1).collect();

        let linear = median_us(9, 200, || {
            let mut acc = 0usize;
            for r in &runs {
                // span_scale_for_range / span_covering_range predicate, ×1 (the
                // emit path runs ~3 such scans per run; one here, scale ×3).
                if let Some(idx) = spans
                    .iter()
                    .position(|s| s.start <= r.start && s.end >= r.end)
                {
                    acc += idx;
                }
            }
            black_box(acc);
        });

        // Indexed: a byte→span-index map built once, O(1) per lookup.
        let indexed = median_us(9, 200, || {
            let end = spans.iter().map(|s| s.end).max().unwrap_or(0);
            let mut map = vec![usize::MAX; end];
            for (i, s) in spans.iter().enumerate() {
                map[s.start..s.end].fill(i);
            }
            let mut acc = 0usize;
            for r in &runs {
                let idx = map.get(r.start).copied().unwrap_or(usize::MAX);
                if idx != usize::MAX {
                    acc += idx;
                }
            }
            black_box(acc);
        });

        eprintln!(
            "   spans=N={n:<4} linear(1 scan/run)={linear:>8.3} µs   indexed(build+lookup)={indexed:>8.3} µs"
        );
    }
}

fn main() {
    bench_end_to_end();
    eprintln!();
    bench_isolated_scan();
}
