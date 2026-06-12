// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Private helpers that look up per-span properties for a given glyph-run text range.

use std::ops::Range;

use crate::color::LayoutColor;

use super::types::{StyleSpan, VerticalAlign};

/// Returns the highlight colour for the first span fully containing
/// `text_range`, or `None` if no such span has a highlight.
pub(super) fn span_highlight_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<LayoutColor> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.highlight_color)
}

/// Returns the link URL for the first span fully containing `text_range`,
/// or `None` if no span in that range carries a link URL.
pub(super) fn span_link_url_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<String> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.link_url.clone())
}

/// Returns the font name for the first span fully containing `text_range`,
/// or `None` if no span in that range carries a font name.
#[cfg(debug_assertions)]
pub(super) fn span_font_name_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<String> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.font_name.clone())
}

/// Returns `true` if the first span fully containing `text_range` has
/// `shadow = true`.
pub(super) fn span_has_shadow(spans: &[StyleSpan], text_range: Range<usize>) -> bool {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .is_some_and(|s| s.shadow)
}

/// Returns the vertical alignment and original (pre-reduction) font size for
/// the first span fully containing `text_range`, or `None` if no vertical
/// alignment is set on that span.
pub(super) fn span_vertical_align_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<(VerticalAlign, f32)> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.vertical_align.map(|va| (va, s.font_size)))
}
