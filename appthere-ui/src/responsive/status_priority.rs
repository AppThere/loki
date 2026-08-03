// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Width-driven **status-bar** item priority (Spec 08 T7.1).
//!
//! The bar drops its lowest-priority items into an overflow popover until the
//! rest fit the measured width, keeping a declared **retention set** — the page
//! indicator and the zoom control — on screen at every width.
//!
//! # Why not the breakpoint
//!
//! The bar previously dropped the word count and the language label whenever
//! [`Breakpoint::is_compact`](crate::responsive::Breakpoint::is_compact) said
//! so. That is a tier decision standing in for a width one, and it is wrong in
//! both directions: a Compact window with three short labels drops items that
//! would have fit, and an Expanded one with a long language name and a
//! seven-digit page label overflows without dropping anything. Same Decision D3
//! as the ribbon cascade — *collapse is width-driven, not tier-driven* — applied
//! to the other end of the window.
//!
//! # What "measured" means here, precisely
//!
//! The **available** width is measured (the viewport sensor). Each item's own
//! width is a **declaration** — [`estimate_label_px`] for text, a constant for
//! the fixed controls — exactly as [`super::ribbon_collapse`] declares group
//! widths rather than asking Blitz per element. A declaration that is a little
//! wrong moves the threshold at which an item drops; it cannot make the bar show
//! something it decided to hide. Getting it wrong in the *under*-estimating
//! direction is the one that costs an overflow, which is why
//! [`estimate_label_px`] weights wide scripts rather than counting `char`s flat.
//!
//! # Retention is structural, not a rule
//!
//! Items in the retention set are not in the drop order at all — they cannot be
//! dropped by a loop that runs one step too far, because there is no step that
//! reaches them. `dropped` is bounded by the droppable count by construction.
//! If the retained items alone exceed the width, [`StatusFit::scroll`] says so;
//! nothing hides the page indicator to make room.
//!
//! # Hysteresis
//!
//! As with the ribbon: drop the instant the strip overflows, restore only when
//! the less-dropped layout clears the width by
//! [`RIBBON_COLLAPSE_HYSTERESIS_PX`], so dragging a window across a threshold
//! does not thrash. Idempotent at a fixed width.

use crate::tokens::layout::{RIBBON_COLLAPSE_HYSTERESIS_PX, RIBBON_OVERFLOW_BUTTON_PX};

/// Mean glyph advance as a fraction of font size, for a narrow-ish UI sans at
/// mixed case with digits and spaces.
///
/// Calibrated by eye against Atkinson Hyperlegible at `FONT_SIZE_XS`, not
/// measured from font tables — see the module docs on what a declaration buys.
const NARROW_ADVANCE_RATIO: f32 = 0.55;

/// Advance ratio for scripts whose glyphs occupy a full em — CJK ideographs,
/// kana, Hangul, and the fullwidth forms.
///
/// Without this a Japanese `language_label` declares roughly half the width it
/// paints, and the bar overflows instead of dropping — the one direction of
/// estimation error that costs a visible defect rather than a slightly early
/// drop.
const WIDE_ADVANCE_RATIO: f32 = 1.0;

/// `true` for characters that paint at roughly one em.
fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F      // Hangul Jamo initial consonants
        | 0x2E80..=0x303E    // CJK radicals, Kangxi, CJK symbols and punctuation
        | 0x3041..=0x33FF    // Hiragana, Katakana, Bopomofo, Hangul Compat, CJK compat
        | 0x3400..=0x4DBF    // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF    // CJK Unified Ideographs
        | 0xA000..=0xA4CF    // Yi
        | 0xAC00..=0xD7A3    // Hangul syllables
        | 0xF900..=0xFAFF    // CJK compatibility ideographs
        | 0xFE30..=0xFE6F    // CJK compatibility forms
        | 0xFF00..=0xFF60    // Fullwidth forms
        | 0xFFE0..=0xFFE6    // Fullwidth signs
        | 0x1F300..=0x1F64F  // Emoji (pictographs, emoticons)
        | 0x20000..=0x2FA1F  // CJK Extension B..F and compatibility supplement
    )
}

/// A declared width (CSS px) for the text `label` rendered at `font_size_px`.
///
/// An estimate from character advances, not a glyph measurement: see the module
/// docs. Wide-script characters count as a full em, narrow ones as
/// [`NARROW_ADVANCE_RATIO`] of one.
#[must_use]
pub fn estimate_label_px(label: &str, font_size_px: f32) -> f32 {
    let units: f32 = label
        .chars()
        .map(|c| {
            if is_wide(c) {
                WIDE_ADVANCE_RATIO
            } else {
                NARROW_ADVANCE_RATIO
            }
        })
        .sum();
    units * font_size_px
}

/// One status-bar item as the fit engine sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusItem {
    /// Higher stays visible longer. Lower-priority items drop first; ties break
    /// by original left-to-right order.
    pub priority: u8,
    /// Declared width when shown in the bar (CSS px), gap included by the caller.
    pub width_px: f32,
    /// `true` for the minimum retention set (page indicator, zoom). A retained
    /// item is never offered to the drop order, so no amount of narrowing hides
    /// it.
    pub retained: bool,
}

/// The resolved fit for one status bar.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusFit {
    /// Per item, in the caller's original order: `true` = render in the bar,
    /// `false` = render inside the overflow popover.
    pub shown: Vec<bool>,
    /// How many items were dropped. Carry back as `prev_dropped` for hysteresis.
    pub dropped: usize,
    /// Whether the overflow trigger is rendered (at least one item dropped).
    pub overflow: bool,
    /// Whether the retained items *still* exceed the width — the floor. Nothing
    /// further can be dropped; the bar is simply narrower than its minimum.
    pub scroll: bool,
}

/// Indices of `items` in drop order — ascending priority, ties by original
/// order — **excluding retained items entirely**.
fn drop_order(items: &[StatusItem]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).filter(|&i| !items[i].retained).collect();
    order.sort_by_key(|&i| (items[i].priority, i));
    order
}

/// Per-item visibility after dropping the first `dropped` entries of `order`.
fn shown_at(items: &[StatusItem], order: &[usize], dropped: usize) -> Vec<bool> {
    let mut shown = vec![true; items.len()];
    for &idx in order.iter().take(dropped) {
        shown[idx] = false;
    }
    shown
}

/// Occupied width for a visibility set, including the overflow trigger once when
/// anything is hidden.
fn bar_width(items: &[StatusItem], shown: &[bool]) -> f32 {
    let mut width = 0.0;
    let mut any_hidden = false;
    for (item, &visible) in items.iter().zip(shown) {
        if visible {
            width += item.width_px;
        } else {
            any_hidden = true;
        }
    }
    if any_hidden {
        width += RIBBON_OVERFLOW_BUTTON_PX;
    }
    width
}

/// Resolves which status-bar items fit `available_px`, given the previously
/// resolved `prev_dropped` (pass `0` on first layout).
///
/// An unmeasured width (`<= 0`) holds `prev_dropped` unchanged — there is
/// nothing to decide yet, and treating "not yet measured" as "zero space" would
/// hide the whole bar on the first frame.
#[must_use]
pub fn resolve_status_fit(
    items: &[StatusItem],
    available_px: f32,
    prev_dropped: usize,
) -> StatusFit {
    let order = drop_order(items);
    let max_dropped = order.len();
    let width_at = |dropped: usize| bar_width(items, &shown_at(items, &order, dropped));

    let mut dropped = prev_dropped.min(max_dropped);
    if available_px > 0.0 {
        // Drop further while the bar overflows and droppable items remain.
        while dropped < max_dropped && width_at(dropped) > available_px {
            dropped += 1;
        }
        // Restore while the next-fuller layout clears the width by the band.
        while dropped > 0 && width_at(dropped - 1) + RIBBON_COLLAPSE_HYSTERESIS_PX <= available_px {
            dropped -= 1;
        }
    }

    let shown = shown_at(items, &order, dropped);
    StatusFit {
        overflow: dropped > 0,
        scroll: available_px > 0.0 && width_at(max_dropped) > available_px,
        shown,
        dropped,
    }
}

#[cfg(test)]
#[path = "status_priority_tests.rs"]
mod tests;
