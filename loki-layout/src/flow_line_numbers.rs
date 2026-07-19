// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Margin line numbering (`w:lnNumType`, gap: Appendix line numbers).
//!
//! A section that carries [`LineNumbering`] prints a number in the left margin
//! beside each line of body text. The counter advances per placed line and (for
//! `restart="newPage"`) resets at each page boundary; `count_by` selects which
//! lines actually show a number. Numbers are emitted as ordinary content items
//! at a negative content-local x (the painters composite content offset by the
//! left margin with no content clip, so negative x lands in the margin).
//!
//! Scope: paginated, single-column, non-table body paragraphs — Word's own
//! defaults (tables and header/footer lines are not numbered).

use loki_doc_model::layout::page::{LineNumberRestart, LineNumbering};

use crate::color::LayoutColor;
use crate::para::{ParagraphLayout, ResolvedParaProps, StyleSpan, layout_paragraph};

use super::FlowState;

/// Font size (pt) for the margin numbers — slightly smaller than body text,
/// matching Word's default Line Number style.
const NUM_FONT_SIZE: f32 = 9.0;

/// Default gap (pt) between a number's right edge and the text when the section
/// gives no explicit `@w:distance` (Word's "Auto").
const DEFAULT_DISTANCE: f32 = 18.0;

/// Per-section line-numbering counter carried on [`FlowState`].
pub(crate) struct LineNumberState {
    config: LineNumbering,
    /// The number the next placed line will receive.
    next: i32,
}

impl LineNumberState {
    /// Build the initial state for a section, seeded at its start value.
    pub(super) fn new(config: &LineNumbering) -> Self {
        Self {
            config: config.clone(),
            next: config.start,
        }
    }

    /// Reset to the start value at a page boundary when `restart="newPage"`.
    pub(super) fn restart_for_page(&mut self) {
        if self.config.restart == LineNumberRestart::NewPage {
            self.next = self.config.start;
        }
    }
}

/// A number to paint: its string and the page-content-local baseline y.
struct PendingNumber {
    text: String,
    baseline_y: f32,
}

/// Emit margin numbers for the lines of `para_layout` whose paragraph-local top
/// lies in `[y0, y1)`, placed at page-content-local offset `ty` (so a
/// paragraph-local y maps to page-local `y + ty`). Advances the section counter
/// once per line regardless of whether the line shows a number.
pub(super) fn emit(
    state: &mut FlowState,
    para_layout: &ParagraphLayout,
    ty: f32,
    y0: f32,
    y1: f32,
) {
    // Gate: active feature, paginated single-column body (not a table cell).
    if state.line_num.is_none()
        || !state.mode.is_paginated()
        || state.columns != 1
        || state.break_long_words
    {
        return;
    }

    // Line-0 ascent (baseline below the paragraph top) — reused for every line as
    // a uniform estimate, so each line's baseline ≈ its top + this delta.
    let ascent = para_layout
        .line_boundaries
        .first()
        .map_or(para_layout.first_baseline, |&(top, _)| {
            para_layout.first_baseline - top
        });

    // Collect the numbers to paint while advancing the counter (mutable borrow of
    // `line_num` only), then shape them (mutable borrow of `resources`) below.
    let mut pending: Vec<PendingNumber> = Vec::new();
    {
        let ln = match &mut state.line_num {
            Some(ln) => ln,
            None => return,
        };
        let count_by = ln.config.count_by.max(1) as i32;

        // An empty paragraph still counts as one line at its own top.
        if para_layout.line_boundaries.is_empty() {
            if 0.0 >= y0 && 0.0 < y1 {
                let n = ln.next;
                ln.next += 1;
                if n.rem_euclid(count_by) == 0 {
                    pending.push(PendingNumber {
                        text: n.to_string(),
                        baseline_y: ty + para_layout.first_baseline,
                    });
                }
            }
        } else {
            for &(top, bottom) in &para_layout.line_boundaries {
                // Membership by the line's midpoint: `top` (Parley's min_coord)
                // can sit slightly above the paragraph origin (negative) and the
                // last line's `bottom` can round just past the paragraph height,
                // so a top/bottom test would mis-bucket the first or last line of
                // a fragment. The midpoint lies strictly inside the line, so it
                // falls in exactly one fragment's [y0, y1) range.
                let mid = (top + bottom) * 0.5;
                if mid < y0 || mid >= y1 {
                    continue;
                }
                let n = ln.next;
                ln.next += 1;
                if n.rem_euclid(count_by) == 0 {
                    pending.push(PendingNumber {
                        text: n.to_string(),
                        baseline_y: ty + top + ascent,
                    });
                }
            }
        }
    }
    if pending.is_empty() {
        return;
    }

    let distance = state
        .line_num
        .as_ref()
        .and_then(|ln| ln.config.distance)
        .map_or(DEFAULT_DISTANCE, |d| d.value() as f32);

    for p in pending {
        paint_number(state, &p, distance);
    }
}

/// Shape one number and push its glyphs into the current page, right-aligned so
/// its right edge sits `distance` pt left of the text margin.
fn paint_number(state: &mut FlowState, p: &PendingNumber, distance: f32) {
    let span = number_span(&p.text);
    let props = ResolvedParaProps::default();
    let num_layout = layout_paragraph(
        state.resources,
        &p.text,
        &[span],
        &props,
        1_000.0,
        state.display_scale,
        false,
    );
    // Right edge at content-local x = -distance; shift left by the number width.
    let dx = -distance - num_layout.width;
    // Align the number's own baseline to the target line baseline.
    let dy = p.baseline_y - num_layout.first_baseline;
    for mut item in num_layout.items {
        item.translate(dx, dy);
        state.current_items.push(item);
    }
}

/// A single black, default-font style span covering the whole number string.
fn number_span(text: &str) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: None,
        font_size: NUM_FONT_SIZE,
        bold: false,
        weight: 400,
        italic: false,
        color: LayoutColor::BLACK,
        underline: None,
        strikethrough: None,
        line_height: None,
        vertical_align: None,
        highlight_color: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        link_url: None,
        math: None,
        scale: None,
        kerning: None,
        baseline_shift: None,
        language: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::layout::page::LineNumbering;

    #[test]
    fn seeds_at_start_value() {
        let cfg = LineNumbering {
            start: 5,
            ..Default::default()
        };
        assert_eq!(LineNumberState::new(&cfg).next, 5);
    }

    #[test]
    fn new_page_restart_resets_to_start() {
        let cfg = LineNumbering {
            start: 1,
            restart: LineNumberRestart::NewPage,
            ..Default::default()
        };
        let mut st = LineNumberState::new(&cfg);
        st.next = 17; // advanced across a page
        st.restart_for_page();
        assert_eq!(st.next, 1, "newPage restarts each page");
    }

    #[test]
    fn continuous_restart_does_not_reset() {
        let cfg = LineNumbering {
            start: 1,
            restart: LineNumberRestart::Continuous,
            ..Default::default()
        };
        let mut st = LineNumberState::new(&cfg);
        st.next = 42;
        st.restart_for_page();
        assert_eq!(
            st.next, 42,
            "continuous numbering never resets on a page break"
        );
    }
}
