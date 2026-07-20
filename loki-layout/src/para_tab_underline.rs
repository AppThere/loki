// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Underline decoration across an underlined tab's expansion gap.
//!
//! A run that is only a `\t` carrying `w:u` (the classic signature line:
//! `<w:r><w:rPr><w:u w:val="single"/></w:rPr><w:tab/></w:r>`) draws its
//! underline across the whole tab gap in Word. Loki excludes `\t` from the
//! Parley text (gap #8), so such a run maps to a zero-length style span and
//! Parley — which only strokes underlines beneath real glyphs — draws nothing.
//! This module recovers the underline from the spans and emits it across the
//! tab box the flow engine opened, matching Word. It also covers a tab embedded
//! inside a longer underlined run, where Word likewise fills the gap.

use loki_doc_model::style::props::tab_stop::TabLeader;

use crate::color::LayoutColor;
use crate::items::{DecorationKind, PositionedDecoration, PositionedItem};
use crate::para_emit::underline_deco_style;

use super::{StyleSpan, UnderlineStyle};

/// Emit a tab inline box's decorations across the gap `[x0, x0 + width]` it
/// opened at `baseline`: the stop's `leader` (when present) and, for an
/// underlined tab run at clean offset `p`, its underline rule. Parley draws
/// neither — the `\t` is excluded from its text (gap #8).
pub(super) fn emit_tab_box(
    items: &mut Vec<PositionedItem>,
    spans: &[StyleSpan],
    p: Option<usize>,
    leader: Option<TabLeader>,
    x0: f32,
    width: f32,
    baseline: f32,
) {
    let x1 = x0 + width;
    if let Some(leader) = leader {
        super::tabs::emit_tab_leader(items, leader, x0, x1, baseline);
    }
    if let Some(p) = p
        && let Some((style, color, fs)) = tab_underline(spans, p)
    {
        emit_tab_underline(items, style, color, fs, x0, x1, baseline);
    }
}

/// The underline (style, colour, font size) that applies to the tab whose box
/// site is at clean-text offset `p`, or `None` when the tab is not underlined.
///
/// Prefers the tab's own run — a zero-length span at `p` (the standalone
/// signature tab) — then falls back to a longer span covering `p` (a tab inside
/// underlined body text). Font size rides along so the rule can be positioned
/// from font metrics (no glyph run exists on a tab-only line to measure).
pub(super) fn tab_underline(
    spans: &[StyleSpan],
    p: usize,
) -> Option<(UnderlineStyle, LayoutColor, f32)> {
    spans
        .iter()
        .find(|s| s.range.start == p && s.range.end == p && s.underline.is_some())
        .or_else(|| {
            spans
                .iter()
                .find(|s| s.range.start <= p && p < s.range.end && s.underline.is_some())
        })
        .and_then(|s| s.underline.map(|u| (u, s.color, s.font_size)))
}

/// Emit the underline rule across a tab gap `[x0, x1]` at `baseline`.
///
/// The rule sits just below the baseline, sourced from the run's `font_size`
/// (Parley `RunMetrics` are unavailable — the line carries no glyph run). Draws
/// nothing for a sub-pixel gap.
pub(super) fn emit_tab_underline(
    items: &mut Vec<PositionedItem>,
    style: UnderlineStyle,
    color: LayoutColor,
    font_size: f32,
    x0: f32,
    x1: f32,
    baseline: f32,
) {
    let width = x1 - x0;
    if width < 1.0 {
        return;
    }
    items.push(PositionedItem::Decoration(PositionedDecoration {
        x: x0,
        // Screen-y is down: a positive offset places the rule below the baseline
        // (≈ the underline position Parley derives for the same face/size).
        y: baseline + font_size * 0.12,
        width,
        thickness: (font_size * 0.06).max(0.75),
        kind: DecorationKind::Underline,
        style: underline_deco_style(style),
        color,
    }));
}

#[cfg(test)]
#[path = "para_tab_underline_tests.rs"]
mod tests;
