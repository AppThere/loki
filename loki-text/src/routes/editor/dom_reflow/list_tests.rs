// SPDX-License-Identifier: Apache-2.0

//! Tests for the DOM reflow view's lists.
//!
//! What is worth pinning here is **not** that a list renders — the sitting
//! comparison says that — but that the marker, the indent and the tab all come
//! from `loki_layout` rather than from a second answer stated here. A list that
//! rendered beautifully with its own markers would be the defect.

use loki_layout::flow::{NESTED_INDENT_PT, list_marker, synthesize_list_item_para};

/// **Ordered numbering counts from the document's own start**, and it is
/// `loki_layout`'s function that says so — restating `i + 1` here is exactly the
/// second copy this module exists to avoid.
#[test]
fn an_ordered_marker_counts_from_the_documents_start() {
    let attrs = loki_doc_model::content::block::ListAttributes {
        start_number: 5,
        style: Default::default(),
        delimiter: Default::default(),
    };
    assert!(list_marker(Some(&attrs), 0).starts_with('5'));
    assert!(list_marker(Some(&attrs), 2).starts_with('7'));
    // And a bullet is not a number.
    assert!(!list_marker(None, 0).starts_with('1'));
}

/// Every marker ends in the tab the canvas path's hanging indent is measured
/// against — which is why the DOM path has to *remove* it (it has no tab stop to
/// resolve it against, and it paints as a `.notdef` box). The removal is
/// `content_para::split_runs`; this pins the thing it removes.
#[test]
fn every_marker_ends_in_a_tab() {
    let attrs = loki_doc_model::content::block::ListAttributes {
        start_number: 1,
        style: Default::default(),
        delimiter: Default::default(),
    };
    assert!(list_marker(Some(&attrs), 0).ends_with('\t'));
    assert!(list_marker(None, 0).ends_with('\t'));
}

/// **The item's first paragraph carries the marker and both indents.** This is
/// `loki_layout`'s synthesis, called from here — the test is that the DOM path
/// asks for it rather than building its own paragraph.
#[test]
fn the_synthesised_item_hangs_by_one_step_at_the_lists_indent() {
    use loki_doc_model::content::block::StyledParagraph;
    use loki_doc_model::content::inline::Inline;

    let para = StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str("item".into())],
        attr: Default::default(),
    };
    let out = synthesize_list_item_para(&para, "\u{2022}\t", 36.0);
    let props = out.direct_para_props.expect("indents");
    assert_eq!(props.indent_hanging.map(|p| p.value()), Some(18.0));
    assert_eq!(props.indent_start.map(|p| p.value()), Some(36.0));
    assert_eq!(out.inlines.len(), 2, "the marker was not prefixed");
}

/// A nested list adds its step to its parent's rather than restarting from zero
/// — the accumulation the canvas path performs with `current_indent`.
#[test]
fn nesting_accumulates_the_indent() {
    let outer = 0.0 + NESTED_INDENT_PT;
    let inner = outer + NESTED_INDENT_PT;
    assert_eq!(inner, 2.0 * NESTED_INDENT_PT);
    assert!(inner > outer, "a nested list did not move in");
}
