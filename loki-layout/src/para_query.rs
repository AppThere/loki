// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Read-only geometry queries on a [`super::ParagraphLayout`]: point hit-test,
//! line-end offset, caret rect, selection rects, and the Option-B y-range item
//! filter (`items_in_y_range`, deferred-feature 6.3) used by split-paragraph
//! fragment emission.
//!
//! A child module of `para`, so it reaches `ParagraphLayout`'s private fields
//! (`parley_layout`, `line_boundaries`, `orig_to_clean`, `clean_to_orig`).

use parley::{Cursor, Selection};

use crate::geometry::LayoutRect;

use super::{Affinity, CursorRect, HitTestResult};

impl super::ParagraphLayout {
    /// Returns the character byte offset closest to the given point in
    /// paragraph-local coordinates.
    ///
    /// Returns `None` when hit-test data is not available, i.e. when the
    /// layout was produced with `preserve_for_editing: false` (read-only mode).
    pub fn hit_test_point(&self, x: f32, y: f32) -> Option<HitTestResult> {
        let layout = self.parley_layout.as_deref()?;
        // Derive the line index from `line_boundaries`: find the first line
        // whose `max_coord` is strictly above the hit y, or clamp to the last line.
        let line_index = self
            .line_boundaries
            .iter()
            .position(|&(_, max_y)| y < max_y)
            .unwrap_or_else(|| self.line_boundaries.len().saturating_sub(1));
        // Glyphs are drawn shifted right by the line's indent, but the Parley
        // layout is un-indented — remove the indent before hit-testing so a
        // click on the visible text maps to the right offset.
        let local_x = x - self.line_indent(line_index);
        let cursor = Cursor::from_point(layout, local_x, y);
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
        // Add the line's indent so the caret sits with the drawn glyphs (the
        // Parley layout is built in an un-indented coordinate space). The line is
        // located from the caret's vertical centre, matching `hit_test_point`.
        let probe_y = y + height * 0.5;
        let line_index = self
            .line_boundaries
            .iter()
            .position(|&(_, max_y)| probe_y < max_y)
            .unwrap_or_else(|| self.line_boundaries.len().saturating_sub(1));
        Some(CursorRect {
            x: bb.x0 as f32 + self.line_indent(line_index),
            y,
            height,
        })
    }

    /// Selection highlight rectangles (paragraph-local layout points) covering
    /// the byte range `[start, end)`, one or more per visual line.  Empty when
    /// the range is collapsed, out of editing mode, or has no glyphs.
    ///
    /// Byte offsets are clamped into range. Used for selection painting in both
    /// view modes via [`crate::ContinuousLayout::selection_rects`].
    pub fn selection_rects(&self, start: usize, end: usize) -> Vec<LayoutRect> {
        let Some(layout) = self.parley_layout.as_deref() else {
            return Vec::new();
        };
        let to_clean = |b: usize| {
            self.orig_to_clean
                .get(b)
                .copied()
                .unwrap_or_else(|| self.orig_to_clean.last().copied().unwrap_or(0))
        };
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let anchor = Cursor::from_byte_index(layout, to_clean(lo), parley::Affinity::Downstream);
        let focus = Cursor::from_byte_index(layout, to_clean(hi), parley::Affinity::Downstream);
        Selection::new(anchor, focus)
            .geometry(layout)
            .into_iter()
            .map(|(bb, line)| {
                LayoutRect::new(
                    bb.x0 as f32 + self.line_indent(line),
                    bb.y0 as f32,
                    (bb.x1 - bb.x0) as f32,
                    (bb.y1 - bb.y0) as f32,
                )
            })
            .collect()
    }

    /// Clones only the items likely to intersect para-local y ∈ `[y0, y1)` —
    /// the Option-B y-range filter (deferred-feature 6.3). Used when emitting a
    /// split-paragraph page fragment so each fragment carries roughly its own
    /// lines' items instead of the whole paragraph's (Option A clips the full
    /// copy on the GPU; the clip stays, so over-inclusion here is harmless).
    ///
    /// The filter is deliberately conservative: each item's vertical extent is
    /// over-estimated (a glyph run's ink is bounded by ±[`Self::GLYPH_Y_SLOP`]
    /// font-sizes around its baseline; groups use their clip/content boxes), and
    /// an item with an unknown extent is always kept — dropping a visible item
    /// would be a rendering bug, keeping an invisible one only wastes the clip.
    pub fn items_in_y_range(&self, y0: f32, y1: f32) -> Vec<crate::items::PositionedItem> {
        use crate::items::PositionedItem as PI;
        self.items
            .iter()
            .filter(|item| {
                let (top, bottom) = match item {
                    PI::GlyphRun(r) => (
                        r.origin.y - r.font_size * Self::GLYPH_Y_SLOP,
                        r.origin.y + r.font_size * Self::GLYPH_Y_SLOP,
                    ),
                    PI::FilledRect(r) | PI::HorizontalRule(r) => {
                        (r.rect.origin.y, r.rect.origin.y + r.rect.size.height)
                    }
                    PI::BorderRect(r) => (r.rect.origin.y, r.rect.origin.y + r.rect.size.height),
                    PI::Image(r) => (r.rect.origin.y, r.rect.origin.y + r.rect.size.height),
                    PI::Decoration(d) => (d.y, d.y + d.thickness),
                    PI::ClippedGroup { clip_rect, .. } => (
                        clip_rect.origin.y,
                        clip_rect.origin.y + clip_rect.size.height,
                    ),
                    // Unknown extent (rotated content, future variants): keep.
                    _ => return true,
                };
                bottom >= y0 && top < y1
            })
            .cloned()
            .collect()
    }

    /// How far a glyph run's ink may plausibly extend above/below its baseline,
    /// in multiples of the font size. Generous on purpose (real ascent+descent
    /// stay within ~1.5 em even with stacked diacritics): the cost of keeping an
    /// extra clipped-away run is trivial, the cost of dropping a visible one is
    /// a rendering bug.
    const GLYPH_Y_SLOP: f32 = 3.0;

    /// Horizontal indent (points) applied to the drawn glyphs of visual line
    /// `line_index`, matching the `indent_x` used when emitting glyph runs: the
    /// first line of a hanging-indent paragraph starts `indent_hanging` to the
    /// left of `indent_start`. Editing geometry adds this so cursor, hit-test,
    /// and selection coordinates line up with the rendered text.
    fn line_indent(&self, line_index: usize) -> f32 {
        let base = if line_index == 0 && self.indent_hanging > 0.0 {
            self.indent_start - self.indent_hanging
        } else {
            self.indent_start
        };
        // Leading lines beside a dropped initial / float band are shifted right.
        if line_index < self.drop_lines {
            base + self.drop_shift
        } else {
            base
        }
    }
}
