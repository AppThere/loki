// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pure measures and predicates used by the column-balancing search, split out
//! of `flow_balance.rs` for the 300-line ceiling: whether two flowed pages
//! carry the same content, whether a section asks for columns at all, and the
//! search's upper bound. None of them touch `FlowState` — they read a
//! `Section` or an already-produced `LayoutPage`, which is why they separate
//! cleanly from the flow probes that call them.

use loki_doc_model::layout::section::Section;

use crate::resolve::pts_to_f32;

/// Whether two pages carry the same content by cheap structural digest: equal
/// item counts and equal (recursive) glyph-run counts. Floats are not compared
/// — identical inputs produce identical counts, which is all the verification
/// needs to reject a mid-block tail (it re-places the whole block, changing
/// both counts).
pub(super) fn pages_match(a: &crate::result::LayoutPage, b: &crate::result::LayoutPage) -> bool {
    a.content_items.len() == b.content_items.len()
        && count_glyph_runs(&a.content_items) == count_glyph_runs(&b.content_items)
}

/// Recursively counts glyph runs, descending into clipped groups.
fn count_glyph_runs(items: &[crate::items::PositionedItem]) -> usize {
    items
        .iter()
        .map(|i| match i {
            crate::items::PositionedItem::GlyphRun(_) => 1,
            crate::items::PositionedItem::ClippedGroup { items, .. } => count_glyph_runs(items),
            _ => 0,
        })
        .sum()
}

/// Whether the section requests two or more columns.
pub(super) fn is_multicolumn(section: &Section) -> bool {
    section
        .layout
        .columns
        .as_ref()
        .is_some_and(|c| c.count >= 2)
}

/// The full per-page content height (page height minus vertical margins), the
/// same value `new_flow_state` derives — the upper bound of the search.
pub(super) fn full_content_height(section: &Section) -> f32 {
    let pl = &section.layout;
    let page_h = pts_to_f32(pl.page_size.height);
    let vmargin = pts_to_f32(pl.margins.top) + pts_to_f32(pl.margins.bottom);
    (page_h - vmargin).max(0.0)
}
