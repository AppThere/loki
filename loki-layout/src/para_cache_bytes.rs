// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Retained-heap accounting for one cached [`ParagraphLayout`] — extracted from
//! `para_cache.rs` at the 300-line ceiling.
//!
//! This is the quantity [`crate::para_cache::ParaCache`] is bounded by and the
//! one the `loki_text::mem` counters report, so what it counts decides what the
//! bound means.

use crate::color::LayoutColor;
use crate::items::{GlyphEntry, PositionedItem};
use crate::para::ParagraphLayout;

/// `size_of::<parley::ClusterData>()` in parley 0.10 — one cluster per
/// character for Latin prose, and the dominant term by a wide margin.
const PARLEY_CLUSTER_BYTES: usize = 24;
/// `size_of::<parley::LineData>()` (includes a 48-byte `LineMetrics`).
const PARLEY_LINE_BYTES: usize = 104;
/// `size_of::<parley::LineItemData>()` — one per style run per line.
const PARLEY_LINE_ITEM_BYTES: usize = 48;
/// `size_of::<parley::Layout<LayoutColor>>()` (280) plus the `Arc` header (16),
/// standing in for the per-layout fixed cost and the small `styles`/`runs`/
/// `fonts` vectors a single-style paragraph carries.
const PARLEY_BASE_BYTES: usize = 296;

/// Approximate retained heap bytes of one cached [`ParagraphLayout`].
///
/// # What is counted, and why the Parley layout had to be
///
/// The editor lays out with `preserve_for_editing`, so **every** cached entry
/// retains a `parley::Layout` for hit-testing and caret placement. That layout
/// was not counted here until the cache became byte-bounded, and it is not a
/// rounding error: measured against parley 0.10's struct definitions, it is
/// ~26 B/char of live data against ~18.5 B/char for everything else this
/// function walks. Budgeting against the old figure would have been budgeting
/// against roughly a third of the object.
///
/// # Still a floor
///
/// Two knowable costs are excluded. Parley's vectors are `push`-grown and it
/// exposes no capacity accessor or `shrink_to_fit`, so their doubling slack
/// (measured at 1.3–1.6× on a 20 000-character Latin sample) is invisible from
/// outside the crate. And the `Arc`'d font blobs every layout keeps alive are
/// shared between entries, so counting them here would multiply-count them.
/// The figure therefore under-states real residency, and the bound derived from
/// it is correspondingly conservative — which is the safe direction for a
/// cache ceiling, since the error admits more eviction rather than less.
pub(super) fn layout_bytes(l: &ParagraphLayout) -> usize {
    let items: usize = l.items.capacity() * std::mem::size_of::<PositionedItem>()
        + l.items.iter().map(item_bytes).sum::<usize>();
    items
        + l.line_boundaries.capacity() * std::mem::size_of::<(f32, f32)>()
        + l.orig_to_clean.approx_heap_bytes()
        + l.clean_to_orig.approx_heap_bytes()
        + l.parley_layout.as_deref().map_or(0, parley_bytes)
}

/// The glyph-vector bytes of one item (nested groups walked); fixed-size
/// rect/rule items own no counted heap.
fn item_bytes(item: &PositionedItem) -> usize {
    match item {
        PositionedItem::GlyphRun(r) => r.glyphs.capacity() * std::mem::size_of::<GlyphEntry>(),
        PositionedItem::ClippedGroup { items, .. } | PositionedItem::RotatedGroup { items, .. } => {
            items.capacity() * std::mem::size_of::<PositionedItem>()
                + items.iter().map(item_bytes).sum::<usize>()
        }
        _ => 0,
    }
}

/// Retained bytes of a `parley::Layout`, summed from the counts its public API
/// exposes exactly (lines, line items per line, clusters per run).
///
/// Parley publishes no size accessor, so this reconstructs the total from the
/// element sizes above. The traversal is O(clusters), but it runs only on the
/// **miss** path — once per newly shaped paragraph — where it is dwarfed by the
/// shaping that just produced the layout.
fn parley_bytes(layout: &parley::Layout<LayoutColor>) -> usize {
    let mut clusters = 0usize;
    let mut line_items = 0usize;
    for line in layout.lines() {
        line_items += line.len();
        for run in line.runs() {
            clusters += run.len();
        }
    }
    PARLEY_BASE_BYTES
        + clusters * PARLEY_CLUSTER_BYTES
        + layout.len() * PARLEY_LINE_BYTES
        + line_items * PARLEY_LINE_ITEM_BYTES
}
