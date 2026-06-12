// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-level layout using Parley.
//!
//! [`layout_paragraph`] takes a flattened text string with ranged
//! [`StyleSpan`]s and paragraph properties, runs Parley shaping and
//! line-breaking, then converts the result into renderer-agnostic
//! [`PositionedItem`]s whose origins are relative to the paragraph's
//! own `(0, 0)` top-left corner.

mod builder;
mod glyph_emit;
mod layout_result;
mod list_marker;
mod span_helpers;
pub mod types;

pub use layout_result::ParagraphLayout;
pub use list_marker::format_list_marker;
pub use types::{
    Affinity, CursorRect, FontVariant, HitTestResult, ResolvedLineHeight, ResolvedListMarker,
    ResolvedParaProps, ResolvedTabStop, StrikethroughStyle, StyleSpan, UnderlineStyle,
    VerticalAlign,
};
// Re-exported for tests (super::* in para_tests.rs).
pub use crate::items::{BorderEdge, DecorationKind};

use std::sync::Arc;

use parley::{AlignmentOptions, InlineBox, PositionedLayoutItem};

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::items::PositionedItem;

use builder::{clean_text_and_spans, next_tab_stop, push_para_styles};
use glyph_emit::{collect_glyph_runs, prepend_border_and_background};

/// Lay out a single paragraph using Parley.
///
/// `text_content` is the flattened text from all inline runs. `style_spans`
/// maps byte ranges to resolved character properties. `available_width` is
/// the maximum line width in points. `display_scale` is the HiDPI scale
/// factor (use `1.0` for layout-only / headless use).
///
/// When `preserve_for_editing` is `true`, the Parley `Layout` object is
/// retained in [`ParagraphLayout::parley_layout`] so that subsequent editing
/// sessions can call [`ParagraphLayout::hit_test_point`] and
/// [`ParagraphLayout::cursor_rect`]. In read-only rendering mode pass
/// `false` to avoid the memory cost on large documents.
pub fn layout_paragraph(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
) -> ParagraphLayout {
    let (clean_text, mut clean_spans, orig_to_clean, clean_to_orig) =
        clean_text_and_spans(text_content, style_spans);

    for span in &mut clean_spans {
        if let Some(ref name) = span.font_name {
            span.font_name = Some(resources.resolve_font_name(name));
        }
    }

    if clean_text.is_empty() {
        return layout_empty_paragraph(
            resources, para_props, available_width, display_scale,
            preserve_for_editing, orig_to_clean, clean_to_orig,
        );
    }

    // NOTE(indent-hanging-width): Parley 0.6 does not expose per-line width
    // control. Fix requires Parley to expose per-line measure.
    // Tracked: fidelity audit gap #8 (partial).
    let line_w = (available_width - para_props.indent_start - para_props.indent_end).max(0.0);

    let tab_inline_widths = compute_tab_widths(
        resources, &clean_text, &clean_spans, para_props, display_scale, line_w,
    );

    let tab_char_positions: Vec<usize> = clean_text
        .char_indices()
        .filter(|(_, c)| *c == '\t')
        .map(|(i, _)| i)
        .collect();

    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx, &clean_text, display_scale, true,
    );
    push_para_styles(&mut builder, para_props, &clean_spans);
    for (idx, &pos) in tab_char_positions.iter().enumerate() {
        let width = tab_inline_widths.get(idx).copied().unwrap_or(0.0);
        builder.push_inline_box(InlineBox { id: idx as u64, index: pos, width, height: 0.0 });
    }

    let mut layout = builder.build(&clean_text);
    layout.break_all_lines(Some(line_w));
    layout.align(Some(line_w), para_props.alignment, AlignmentOptions::default());

    let total_height = layout.height();
    let total_width = layout.width();
    let first_baseline = layout.lines().next().map(|l| l.metrics().baseline).unwrap_or(0.0);
    let last_baseline = layout.lines().last().map(|l| l.metrics().baseline).unwrap_or(0.0);
    let line_boundaries: Vec<(f32, f32)> = layout
        .lines()
        .map(|l| (l.metrics().min_coord, l.metrics().max_coord))
        .collect();

    let mut items: Vec<PositionedItem> = Vec::new();
    collect_glyph_runs(&layout, &clean_text, &clean_spans, para_props, resources, &mut items);
    prepend_border_and_background(&mut items, para_props, total_width, total_height);

    let parley_layout = if preserve_for_editing { Some(Arc::new(layout)) } else { None };

    ParagraphLayout {
        height: total_height,
        width: total_width,
        items,
        first_baseline,
        last_baseline,
        line_boundaries,
        parley_layout,
        orig_to_clean,
        clean_to_orig,
    }
}

/// Build a layout for an empty paragraph (no glyphs).
fn layout_empty_paragraph(
    resources: &mut FontResources,
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
    orig_to_clean: Vec<usize>,
    clean_to_orig: Vec<usize>,
) -> ParagraphLayout {
    if !preserve_for_editing {
        return ParagraphLayout {
            height: 0.0, width: 0.0, items: vec![], first_baseline: 0.0,
            last_baseline: 0.0, line_boundaries: vec![], parley_layout: None,
            orig_to_clean, clean_to_orig,
        };
    }
    // Build a phantom single-space layout so cursor_rect can return a
    // properly-sized caret for empty paragraphs.
    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx, " ", display_scale, true,
    );
    push_para_styles(&mut builder, para_props, &[]);
    let mut phantom = builder.build(" ");
    phantom.break_all_lines(Some(available_width));
    let first_baseline = phantom.lines().next().map(|l| l.metrics().baseline).unwrap_or(0.0);
    ParagraphLayout {
        height: 0.0, width: 0.0, items: vec![], first_baseline,
        last_baseline: first_baseline, line_boundaries: vec![],
        parley_layout: Some(Arc::new(phantom)),
        orig_to_clean, clean_to_orig,
    }
}

/// Two-pass tab width computation (gap #7).
///
/// Pass 1: probe layout with zero-width InlineBoxes to measure `\t` x-positions.
/// Returns per-tab widths for pass 2 (done in the caller).
fn compute_tab_widths(
    resources: &mut FontResources,
    clean_text: &str,
    clean_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    display_scale: f32,
    line_w: f32,
) -> Vec<f32> {
    let tab_char_positions: Vec<usize> = clean_text
        .char_indices()
        .filter(|(_, c)| *c == '\t')
        .map(|(i, _)| i)
        .collect();
    if tab_char_positions.is_empty() {
        return vec![];
    }
    let mut probe = resources.layout_cx.ranged_builder(
        &mut resources.font_cx, clean_text, display_scale, true,
    );
    push_para_styles(&mut probe, para_props, clean_spans);
    for (idx, &pos) in tab_char_positions.iter().enumerate() {
        probe.push_inline_box(InlineBox { id: idx as u64, index: pos, width: 0.0, height: 0.0 });
    }
    let mut probe_layout = probe.build(clean_text);
    probe_layout.break_all_lines(Some(line_w));
    let mut x_positions = vec![0.0f32; tab_char_positions.len()];
    for line in probe_layout.lines() {
        for item in line.items() {
            if let PositionedLayoutItem::InlineBox(pib) = item {
                let idx = pib.id as usize;
                if idx < x_positions.len() { x_positions[idx] = pib.x; }
            }
        }
    }
    x_positions
        .iter()
        .map(|&x| {
            (next_tab_stop(&para_props.tab_stops, x, para_props.indent_hanging) - x).max(0.0)
        })
        .collect()
}

#[cfg(test)]
#[path = "../para_tests.rs"]
mod tests;
