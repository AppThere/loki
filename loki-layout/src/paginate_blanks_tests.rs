// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for even/odd section-break blank-page insertion.

use super::{blank_page, needs_blank_before};
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::SectionStart;

#[test]
fn first_section_never_gets_a_filler() {
    // page_count == 0 → document start, break type immaterial.
    for start in [
        SectionStart::EvenPage,
        SectionStart::OddPage,
        SectionStart::NewPage,
    ] {
        assert!(!needs_blank_before(start, 0), "{start:?}");
    }
}

#[test]
fn even_page_needs_filler_only_when_next_is_odd() {
    // After 1 page (next = 2, even) an EvenPage section fits — no filler.
    assert!(!needs_blank_before(SectionStart::EvenPage, 1));
    // After 2 pages (next = 3, odd) it must skip to page 4 — filler needed.
    assert!(needs_blank_before(SectionStart::EvenPage, 2));
}

#[test]
fn odd_page_needs_filler_only_when_next_is_even() {
    // After 2 pages (next = 3, odd) an OddPage section fits — no filler.
    assert!(!needs_blank_before(SectionStart::OddPage, 2));
    // After 1 page (next = 2, even) it must skip to page 3 — filler needed.
    assert!(needs_blank_before(SectionStart::OddPage, 1));
}

#[test]
fn new_page_and_continuous_never_need_a_filler() {
    assert!(!needs_blank_before(SectionStart::NewPage, 3));
    assert!(!needs_blank_before(SectionStart::Continuous, 3));
}

#[test]
fn blank_page_is_empty_and_non_editing() {
    let page = blank_page(4, &PageLayout::default());
    assert_eq!(page.page_number, 4);
    assert!(page.content_items.is_empty());
    assert!(page.header_items.is_empty());
    assert!(page.footer_items.is_empty());
    assert!(page.editing_data.is_none());
    assert!(page.page_size.width > 0.0 && page.page_size.height > 0.0);
}
