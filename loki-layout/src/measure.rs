// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The reading **measure** — how wide a line of body text should be, resolved
//! against live font metrics (Spec 08 T7.2, decision D-05).
//!
//! # Why this is not a constant
//!
//! The reflow view capped its column at a fixed 820 CSS px. That is a width, and
//! measure is not a width — it is a **character count**. Typographers set it in
//! characters (66 is the classic single-column figure; 72–80 is comfortable for
//! screen prose) precisely because the width that delivers it depends on the
//! face and the size: 76 characters of 11 pt Georgia and 76 characters of 11 pt
//! Arial Narrow are not the same number of points, and neither is 76 characters
//! of the same face at 14 pt. A constant is right for exactly one face at one
//! size and silently wrong everywhere else — long lines in a condensed face,
//! cramped ones in a wide one.
//!
//! # "Live font metrics" means shaping, not counting
//!
//! [`mean_advance_pt`] builds a real Parley layout for a sample string in the
//! requested family and size and reads its advance. That goes through the same
//! font resolution, fallback and shaping the document text does, so a family
//! that is not installed resolves the same way here as it does on the page, and
//! the measure follows the font that is actually used rather than the one that
//! was asked for.
//!
//! This is deliberately *not* the character-advance estimate
//! `appthere_ui::responsive::estimate_label_px` uses. That one declares chrome
//! widths where no font context is at hand and a small error moves a threshold;
//! here the error would be visible as line length in the reader's face.
//!
//! # The sample, and what it assumes
//!
//! [`PROSE_SAMPLE`] is ordinary lowercase English with its natural share of
//! spaces. Mean advance is its width divided by its character count, so the
//! answer is "how wide is one character of typical prose in this face", which is
//! the quantity measure is defined in terms of.
//!
//! **It is English-biased, and that is a real bound rather than an oversight.**
//! Measure in the typographic sense is a Latin-script concept; CJK sets lines by
//! character count directly, at roughly one em each, and a Latin sample would
//! under-estimate their width by about half. `TODO(measure-cjk)`: resolve the
//! sample from the document's script rather than assuming Latin.

use crate::font::FontResources;

/// The sample whose shaped advance defines one character's mean width.
///
/// Lowercase English prose with its natural spacing — not the alphabet, which
/// has no spaces and so over-states the mean by about a fifth, and not a
/// pangram, which over-weights the rare wide letters it exists to include.
pub const PROSE_SAMPLE: &str = "the quick brown fox jumps over a lazy dog while the rest of the type \
     sets itself into an ordinary line of running text";

/// The default reading measure in characters, when the user has expressed no
/// preference — the middle of T7.2's 72–80 band.
pub const DEFAULT_MEASURE_CHARS: u32 = 76;

/// The narrowest accepted user-set measure, in characters.
///
/// Narrower than any comfortable reading column on purpose: a reader who wants
/// a very narrow column on a phone is not making a mistake.
pub const MIN_MEASURE_CHARS: u32 = 20;

/// The widest accepted user-set measure, in characters.
///
/// Where a line stops being readable at all — past it the eye loses the line it
/// is returning to, which is the defect measure exists to prevent.
pub const MAX_MEASURE_CHARS: u32 = 160;

/// The mean advance of one character of body prose, in points, for `family` at
/// `size_pt` — measured by shaping [`PROSE_SAMPLE`], not estimated.
///
/// Returns `None` when the sample shapes to nothing measurable (no font
/// registered yet, or a zero/invalid size), which is a real answer: there is no
/// measure to compute, and the caller should keep whatever it had rather than
/// substitute a guess.
#[must_use]
pub fn mean_advance_pt(resources: &mut FontResources, family: &str, size_pt: f32) -> Option<f32> {
    if !size_pt.is_finite() || size_pt <= 0.0 {
        return None;
    }
    let chars = PROSE_SAMPLE.chars().count() as f32;
    let width = sample_advance_pt(resources, PROSE_SAMPLE, Some(family), size_pt)?;
    let mean = width / chars;
    (mean.is_finite() && mean > 0.0).then_some(mean)
}

/// The shaped advance of `sample` in `family` at `size_pt`, in points.
///
/// Laid out unconstrained (a single line), so the result is the run's own
/// advance and not a wrapped block's widest line.
///
/// `family` is `None` when the caller has no name to offer — a run that
/// specifies no font — and the sample is then shaped in Parley's own default.
/// Pushing an empty family name instead resolves to nothing and shapes to a
/// zero advance, which reads as "unmeasurable" and would make a caller skip
/// silently rather than measure the face that will actually be used.
pub(crate) fn sample_advance_pt(
    resources: &mut FontResources,
    sample: &str,
    family: Option<&str>,
    size_pt: f32,
) -> Option<f32> {
    use parley::{FontFamily, StyleProperty};

    let resolved = family.map(|f| resources.resolve_font_name(f));
    let FontResources {
        font_cx, layout_cx, ..
    } = resources;
    // Same quantisation as layout: at scale 1.0 snapping would round the
    // advance to a whole point, which for a short run — a list label of a few
    // points — is a large fraction of the value being measured.
    let mut builder = layout_cx.ranged_builder(font_cx, sample, 1.0, crate::QUANTIZE_LAYOUT);
    builder.push_default(StyleProperty::FontSize(size_pt));
    if let Some(resolved) = &resolved {
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(
            resolved.as_str(),
        )));
    }
    let mut layout: parley::Layout<crate::color::LayoutColor> = builder.build(sample);
    // `None` = do not wrap: one line, whose width is the advance being asked for.
    layout.break_all_lines(None);
    let w = layout.width();
    (w.is_finite() && w > 0.0).then_some(w)
}

/// The content width in points that delivers `chars` characters of body prose in
/// `family` at `size_pt`.
///
/// `chars` is clamped to [`MIN_MEASURE_CHARS`]..=[`MAX_MEASURE_CHARS`] — a
/// setting outside the range is a value to bring into it, not a reason to
/// refuse: the caller is a stored preference that may predate the range.
///
/// `None` when the font cannot be measured; see [`mean_advance_pt`].
#[must_use]
pub fn measure_width_pt(
    resources: &mut FontResources,
    family: &str,
    size_pt: f32,
    chars: u32,
) -> Option<f32> {
    let chars = chars.clamp(MIN_MEASURE_CHARS, MAX_MEASURE_CHARS) as f32;
    mean_advance_pt(resources, family, size_pt).map(|mean| mean * chars)
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
