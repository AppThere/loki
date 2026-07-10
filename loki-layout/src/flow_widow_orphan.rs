// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Widow and orphan control for paragraph pagination (fidelity gap 5.9).
//!
//! When a paragraph splits across a page/column boundary, Word and LibreOffice
//! avoid stranding a lone line: an **orphan** is the paragraph's first line left
//! at the bottom of a page, a **widow** its last line carried alone to the top of
//! the next. Both default to a two-line minimum. This module decides, given the
//! natural split line, whether to pull lines down (widow) or defer the whole
//! fragment to the next page (orphan). It is pure — the flow engine
//! ([`super::split_and_place_loop`]) applies the decision.

/// Adjusts the natural split at line `natural_k` for widow/orphan control.
///
/// Returns `Some(k)` to split after line index `k` (0-based into
/// `line_boundaries`) — the natural split, or an earlier one when widow control
/// pulled lines down — or `None` to defer the whole fragment to the next page
/// (orphan control: the current page cannot hold `orphan_min` of the
/// paragraph's first lines).
///
/// `line_boundaries` are the paragraph's per-line `(min_y, max_y)` pairs;
/// `frag_start` is the paragraph-local y of the current fragment's top;
/// `natural_k` is the last line that fits on the current page; `mid_page` is
/// whether there is content above the paragraph on this page (orphan control
/// only defers a paragraph that is not already at a page top).
pub(super) fn resolve_split(
    line_boundaries: &[(f32, f32)],
    frag_start: f32,
    natural_k: usize,
    orphan_min: usize,
    widow_min: usize,
    mid_page: bool,
) -> Option<usize> {
    let total = line_boundaries.len();
    if total == 0 {
        return Some(natural_k);
    }
    // First line index of this fragment (the first line not already emitted).
    let start_line = line_boundaries
        .iter()
        .position(|&(_, max)| max > frag_start)
        .unwrap_or(0);

    let mut k = natural_k;

    // Widow: if the tail carried to the next page is a short final piece
    // (fewer than `widow_min` lines), pull lines down so it keeps `widow_min`.
    // `next_count < widow_min` implies the tail is tiny, so it is the paragraph's
    // final fragment (a longer tail would split again and be handled there).
    let next_count = total.saturating_sub(k + 1);
    if next_count >= 1 && next_count < widow_min {
        let k_widow = total.saturating_sub(widow_min + 1);
        if k_widow >= start_line {
            k = k.min(k_widow);
        }
    }

    // Orphan: never strand fewer than `orphan_min` of the paragraph's first
    // lines at the bottom of a page. Only for the paragraph's first fragment
    // (`start_line == 0`) and only mid-page — a page-top paragraph taller than
    // the page must split as-is.
    if start_line == 0 && mid_page {
        let cur_count = k + 1 - start_line;
        if cur_count < orphan_min {
            return None;
        }
    }

    Some(k)
}

#[cfg(test)]
#[path = "flow_widow_orphan_tests.rs"]
mod tests;
