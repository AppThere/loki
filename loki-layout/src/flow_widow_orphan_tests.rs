// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the pure widow/orphan split resolver. `Some(k)` = split after
//! line `k`; `None` = defer the whole paragraph (orphan control).

use super::resolve_split;

/// `n` uniform 10 pt lines: line `i` spans `(i*10, (i+1)*10)`.
fn lines(n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| (i as f32 * 10.0, (i as f32 + 1.0) * 10.0))
        .collect()
}

#[test]
fn no_adjustment_when_both_sides_are_comfortable() {
    // 8 lines, 5 fit (natural_k = 4): 5 on this page, 3 on the next — no widow.
    assert_eq!(resolve_split(&lines(8), 0.0, 4, 2, 2, false), Some(4));
}

#[test]
fn widow_pulls_a_line_down_to_keep_two_on_the_tail() {
    // 5 lines, 4 fit (natural_k = 3) ⇒ tail would be a lone line 4 (widow).
    // Pull the split back to line 2 so lines 3–4 (two lines) go to the next page.
    assert_eq!(resolve_split(&lines(5), 0.0, 3, 2, 2, false), Some(2));
}

#[test]
fn orphan_defers_the_whole_paragraph_mid_page() {
    // First fragment, only line 0 fits (natural_k = 0), placed mid-page ⇒ a lone
    // first line (orphan): defer the whole paragraph to the next page.
    assert_eq!(resolve_split(&lines(5), 0.0, 0, 2, 2, true), None);
}

#[test]
fn orphan_does_not_defer_at_a_page_top() {
    // Same split but the paragraph already starts a page (nothing above): it must
    // split as-is — deferring would loop.
    assert_eq!(resolve_split(&lines(5), 0.0, 0, 2, 2, false), Some(0));
}

#[test]
fn widow_fix_that_creates_an_orphan_defers_the_whole_paragraph() {
    // 3 lines, 2 fit (natural_k = 1), mid-page. Widow control would pull to line 0
    // (1 here, 2 on the tail), but that strands a lone first line ⇒ defer instead.
    assert_eq!(resolve_split(&lines(3), 0.0, 1, 2, 2, true), None);
}

#[test]
fn disabled_control_keeps_the_natural_split() {
    // orphan_min = widow_min = 0 ⇒ never adjust, never defer.
    assert_eq!(resolve_split(&lines(5), 0.0, 3, 0, 0, true), Some(3));
    assert_eq!(resolve_split(&lines(5), 0.0, 0, 0, 0, true), Some(0));
}

#[test]
fn continuation_fragment_applies_widow_but_not_orphan() {
    // frag_start = 25 ⇒ this fragment begins at line 2 (max 30 > 25). 5 lines,
    // natural_k = 3 leaves a lone line 4 (widow) ⇒ pull to line 2; orphan control
    // does not fire because these are not the paragraph's first lines.
    assert_eq!(resolve_split(&lines(5), 25.0, 3, 2, 2, true), Some(2));
}

#[test]
fn empty_paragraph_is_a_no_op() {
    assert_eq!(resolve_split(&[], 0.0, 0, 2, 2, true), Some(0));
}
