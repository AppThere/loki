// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Mapping of a model `ParaProps` record to the layout `ResolvedParaProps`.
//! Split out of `resolve.rs` (Phase 7.1); `resolve_para_props` (in
//! `resolve.rs`) calls `map_para_props`.

use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::para_props::{
    LineHeight as DocLineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_doc_model::style::props::tab_stop::TabAlignment;
use parley::Alignment;

use crate::geometry::LayoutInsets;
use crate::para::{ResolvedLineHeight, ResolvedListMarker, ResolvedParaProps, ResolvedTabStop};

use super::{convert_border, pts_to_f32, resolve_color};

/// Map a [`Spacing`] variant to a point value; percentage-based spacing
/// falls back to `0.0` (line height is not known at this stage).
#[inline]
fn resolve_spacing(s: Option<Spacing>) -> f32 {
    match s {
        Some(Spacing::Exact(pts)) => pts_to_f32(pts),
        _ => 0.0,
    }
}

/// Map a [`ParaProps`] record to the layout [`ResolvedParaProps`].
pub(super) fn map_para_props(p: &ParaProps) -> ResolvedParaProps {
    ResolvedParaProps {
        alignment: match p.alignment {
            Some(ParagraphAlignment::Right) => Alignment::End,
            Some(ParagraphAlignment::Center) => Alignment::Center,
            Some(ParagraphAlignment::Justify) => Alignment::Justify,
            _ => Alignment::Start,
        },
        space_before: resolve_spacing(p.space_before),
        space_after: resolve_spacing(p.space_after),
        indent_start: p.indent_start.map(pts_to_f32).unwrap_or(0.0),
        indent_end: p.indent_end.map(pts_to_f32).unwrap_or(0.0),
        indent_first_line: p.indent_first_line.map(pts_to_f32).unwrap_or(0.0),
        line_height: p.line_height.and_then(|lh| match lh {
            // IMPORTANT: The OOXML mapper stores Multiple as a ratio, NOT a
            // percentage, despite the doc-model comment (e.g. line=240 →
            // Multiple(1.0), line=360 → Multiple(1.5)). Do NOT divide by 100.
            //
            // lineRule="auto" with line=240 (single spacing) is the most common
            // case. Return None so Parley uses natural font metrics
            // (ascender + descender + leading — exactly what "auto" means).
            // For non-unity multipliers, MetricsRelative scales those natural
            // metrics (1.5 = one-and-a-half spacing, 2.0 = double spacing).
            DocLineHeight::Multiple(m) => {
                if (m - 1.0).abs() < 0.02 {
                    None // Single spacing — let Parley default take over
                } else {
                    Some(ResolvedLineHeight::MetricsRelative(m))
                }
            }
            DocLineHeight::Exact(pts) => Some(ResolvedLineHeight::Exact(pts_to_f32(pts))),
            DocLineHeight::AtLeast(pts) => Some(ResolvedLineHeight::AtLeast(pts_to_f32(pts))),
            // Future variants — fall back to natural metrics.
            _ => None,
        }),
        background_color: p.background_color.as_ref().map(|c| resolve_color(Some(c))),
        border_top: p.border_top.as_ref().and_then(convert_border),
        border_bottom: p.border_bottom.as_ref().and_then(convert_border),
        border_left: p.border_left.as_ref().and_then(convert_border),
        border_right: p.border_right.as_ref().and_then(convert_border),
        padding: LayoutInsets {
            top: p.padding_top.map(pts_to_f32).unwrap_or(0.0),
            right: p.padding_right.map(pts_to_f32).unwrap_or(0.0),
            bottom: p.padding_bottom.map(pts_to_f32).unwrap_or(0.0),
            left: p.padding_left.map(pts_to_f32).unwrap_or(0.0),
        },
        keep_together: p.keep_together.unwrap_or(false),
        keep_with_next: p.keep_with_next.unwrap_or(false),
        // Widow/orphan control. Word/LibreOffice default it ON (2 lines); OOXML's
        // single `w:widowControl` toggle governs both (the mapper sets only
        // `widow_control`), so each side falls back to the other, then to 2. An
        // explicit `0` (control turned off) disables it.
        orphan_min: p.orphan_control.or(p.widow_control).unwrap_or(2),
        widow_min: p.widow_control.or(p.orphan_control).unwrap_or(2),
        page_break_before: p.page_break_before.unwrap_or(false),
        page_break_after: p.page_break_after.unwrap_or(false),
        // NOTE(bidi): `ParaProps.bidi` (RTL paragraph direction) is not forwarded.
        // Parley 0.6 has no `StyleProperty` for text direction and exposes no
        // public bidi level API (`BidiLevel`/`BidiResolver` are pub(crate)).
        // Parley runs BiDi automatically from Unicode character classes, so
        // purely RTL text in RTL scripts will display correctly without explicit
        // direction. Explicit `bidi: true` paragraphs in mixed-direction documents
        // may render incorrectly. Revisit when Parley exposes a direction API.
        // Tracked: fidelity audit gap #19 (deferred).
        indent_hanging: p.indent_hanging.map(pts_to_f32).unwrap_or(0.0),
        list_marker: match (&p.list_id, p.list_level) {
            (Some(id), Some(level)) => Some(ResolvedListMarker {
                list_id: ListId::new(id.as_str()),
                level,
            }),
            _ => None,
        },
        // Tab stops (gap #7): convert from Points to f32, sort ascending,
        // drop Clear entries (already filtered by the OOXML mapper).
        tab_stops: {
            let mut stops: Vec<ResolvedTabStop> = p
                .tab_stops
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|s| s.alignment != TabAlignment::Clear)
                .map(|s| ResolvedTabStop {
                    position: pts_to_f32(s.position),
                    alignment: s.alignment,
                    leader: s.leader,
                })
                .collect();
            stops.sort_by(|a, b| {
                a.position
                    .partial_cmp(&b.position)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            stops
        },
        // Built-in fallback; the flow engine overrides from the document's
        // `DocumentSettings::default_tab_stop_pt` when one is set.
        default_tab_stop: 36.0,
        // Set by the flow engine for table-cell content; see ResolvedParaProps.
        break_long_words: false,
        // Dropped initial (rendered in the read-only/paint path); see
        // `layout_paragraph`. Forwarded straight from the imported model.
        drop_cap: p.drop_cap,
        // Float wrap band is injected by the flow engine, not the model.
        wrap_band: None,
    }
}
