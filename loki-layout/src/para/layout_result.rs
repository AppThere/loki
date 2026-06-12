// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `ParagraphLayout` — the output of a single paragraph layout pass.

use std::sync::Arc;

use parley::Cursor;

use crate::color::LayoutColor;
use crate::items::PositionedItem;

use super::types::{Affinity, CursorRect, HitTestResult};

/// The measured result of laying out one paragraph.
#[derive(Clone)]
pub struct ParagraphLayout {
    /// Total height of this paragraph including internal line spacing.
    /// Does **not** include [`ResolvedParaProps::space_before`] /
    /// [`ResolvedParaProps::space_after`]; those are for the caller.
    pub height: f32,
    /// Maximum line width used (≤ `available_width`).
    pub width: f32,
    /// Positioned items from this paragraph (glyph runs + decorations +
    /// optional background/border). Origins are relative to `(0, 0)`.
    pub items: Vec<PositionedItem>,
    /// Baseline of the first line, measured from the top of the paragraph.
    pub first_baseline: f32,
    /// Baseline of the last line, measured from the top of the paragraph.
    pub last_baseline: f32,
    /// Per-line `(min_coord, max_coord)` in paragraph-local layout units.
    /// Populated from Parley line metrics after `break_all_lines`.
    /// Empty for empty paragraphs.
    ///
    /// Used by `flow_section` to find clean split points at line boundaries.
    /// `TODO(split-optimise)`: Option B y-range item filter can use this field
    /// to avoid rendering clipped content to the GPU once the Option A baseline
    /// is stable and profiled.
    pub line_boundaries: Vec<(f32, f32)>,
    /// Parley layout object retained for hit testing and cursor positioning.
    ///
    /// `None` in read-only rendering mode (when `preserve_for_editing` is
    /// `false` on the `layout_paragraph` call). Populated only when the caller
    /// opts in so that long read-only documents pay no memory cost.
    ///
    /// Wrapped in `Arc` so `ParagraphLayout` remains cheaply cloneable when
    /// the editing layer shares layouts across the page editing index.
    pub parley_layout: Option<Arc<parley::Layout<LayoutColor>>>,
    /// Original to cleaned byte index mappings.
    pub orig_to_clean: Vec<usize>,
    /// Cleaned to original byte index mappings.
    pub clean_to_orig: Vec<usize>,
}

impl std::fmt::Debug for ParagraphLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphLayout")
            .field("height", &self.height)
            .field("width", &self.width)
            .field("items", &self.items)
            .field("first_baseline", &self.first_baseline)
            .field("last_baseline", &self.last_baseline)
            .field("line_boundaries", &self.line_boundaries)
            .field(
                "parley_layout",
                &self.parley_layout.as_ref().map(|_| "<Layout>"),
            )
            .field("orig_to_clean", &self.orig_to_clean)
            .field("clean_to_orig", &self.clean_to_orig)
            .finish()
    }
}

impl ParagraphLayout {
    /// Returns the character byte offset closest to the given point in
    /// paragraph-local coordinates.
    ///
    /// Returns `None` when hit-test data is not available, i.e. when the
    /// layout was produced with `preserve_for_editing: false` (read-only mode).
    pub fn hit_test_point(&self, x: f32, y: f32) -> Option<HitTestResult> {
        let layout = self.parley_layout.as_deref()?;
        let cursor = Cursor::from_point(layout, x, y);
        let byte_offset = cursor.index();
        let mapped_offset = self
            .clean_to_orig
            .get(byte_offset)
            .copied()
            .unwrap_or_else(|| self.clean_to_orig.last().copied().unwrap_or(0));
        let affinity = match cursor.affinity() {
            parley::Affinity::Upstream => Affinity::Upstream,
            parley::Affinity::Downstream => Affinity::Downstream,
        };
        // Derive the line index from `line_boundaries`: find the first line
        // whose `max_coord` is strictly above the hit y, or clamp to the last line.
        let line_index = self
            .line_boundaries
            .iter()
            .position(|&(_, max_y)| y < max_y)
            .unwrap_or_else(|| self.line_boundaries.len().saturating_sub(1));
        Some(HitTestResult {
            byte_offset: mapped_offset,
            affinity,
            line_index,
        })
    }

    /// Returns the byte offset at the end of the visual line that contains
    /// `byte_offset`, optionally trimming a trailing hard-break character.
    ///
    /// `text` is the same UTF-8 string used to build this layout; it is needed
    /// only to check for a trailing `\n` byte that Parley may include in the
    /// line's [`text_range`].  For soft-wrapped lines the range end IS the
    /// correct cursor position (the character sits at the wrap boundary on the
    /// current line with upstream affinity).  For hard-break lines the `\n` is
    /// excluded so the cursor stays after the last visible glyph.
    ///
    /// Returns `None` when hit-test data is not available (read-only mode) or
    /// when the paragraph has no lines.
    pub fn line_end_offset(&self, byte_offset: usize, text: &str) -> Option<usize> {
        let layout = self.parley_layout.as_ref()?;
        let clean_offset = self
            .orig_to_clean
            .get(byte_offset)
            .copied()
            .unwrap_or_else(|| self.orig_to_clean.last().copied().unwrap_or(0));
        // Find the line whose text range contains clean_offset, or fall back to
        // the last line (handles cursor positioned at text.len()).
        let line = layout
            .lines()
            .find(|l| {
                let r = l.text_range();
                r.start <= clean_offset && clean_offset < r.end
            })
            .or_else(|| layout.lines().last())?;

        let range = line.text_range();
        let end = range.end;

        let mapped_end = self
            .clean_to_orig
            .get(end)
            .copied()
            .unwrap_or_else(|| self.clean_to_orig.last().copied().unwrap_or(0));

        // Trim a trailing '\n' or '\r\n' so End lands before the newline byte, not after.
        // In loki-text, paragraph breaks are modelled as separate blocks, so
        // '\n' inside a block's text is unusual — this guard handles edge cases.
        let mut trimmed = mapped_end;
        if trimmed > 0 && text.as_bytes().get(trimmed - 1).copied() == Some(b'\n') {
            trimmed -= 1;
        }
        if trimmed > 0 && text.as_bytes().get(trimmed - 1).copied() == Some(b'\r') {
            trimmed -= 1;
        }

        Some(trimmed)
    }

    /// Returns the visual rectangle for a cursor at the given byte offset in
    /// paragraph-local coordinates.
    ///
    /// Returns `None` when hit-test data is not available (read-only mode).
    /// When `byte_offset` is out of range it is clamped to the nearest valid
    /// position by Parley.
    pub fn cursor_rect(&self, byte_offset: usize) -> Option<CursorRect> {
        let layout = self.parley_layout.as_deref()?;
        let clean_offset = self
            .orig_to_clean
            .get(byte_offset)
            .copied()
            .unwrap_or_else(|| self.orig_to_clean.last().copied().unwrap_or(0));
        let cursor = Cursor::from_byte_index(layout, clean_offset, parley::Affinity::Downstream);
        // width=1.0 requests a 1-point wide caret geometry.
        let bb = cursor.geometry(layout, 1.0);
        let y = bb.y0 as f32;
        let height = (bb.y1 - bb.y0) as f32;
        Some(CursorRect {
            x: bb.x0 as f32,
            y,
            height,
        })
    }
}
