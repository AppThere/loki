// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! How the template gallery is laid out at each size class (Spec 08 T4.4, I-03).
//!
//! Pure, so the decision is testable without a window — which matters more here
//! than usual: the branch under test is *the narrow one*, and a development
//! machine is never narrow. This is T4.0's `LOKI_DEVICE_PROFILE` argument applied
//! to the viewport rather than to the pointer.

use crate::responsive::Breakpoint;
use crate::tokens::spacing::{SPACE_1, SPACE_2, SPACE_3};
use crate::tokens::typography::FONT_SIZE_LABEL;

/// Fixed card width in CSS px. Fixed rather than fractional so a row holds a
/// whole number of cards at every width, which is what makes "two rows" a
/// height a reader can predict rather than a consequence of the wrap point.
pub(crate) const CARD_WIDTH_PX: f32 = 100.0;

/// The format swatch's height.
pub(crate) const SWATCH_HEIGHT_PX: f32 = 72.0;

/// One card's height — **derived, not measured**.
///
/// Swatch, the gap below it, one line of label, and the card's own padding on
/// both sides. Written as a sum of the tokens the card actually uses so a
/// padding change moves the row cap with it; a hand-tuned constant here would
/// be the second source that drifts (L08-029).
///
/// The label is **one line** by construction — `CARD_WIDTH_PX` is wide enough
/// for the built-in names at the label size. A name that wraps makes the card
/// taller than this, which the `Some(max_height)` form handles by scrolling
/// rather than by clipping. That case is exactly what the r48 acceptance line
/// was written about.
pub(crate) const CARD_HEIGHT_PX: f32 =
    SWATCH_HEIGHT_PX + SPACE_2 + FONT_SIZE_LABEL + 2.0 * SPACE_3 + SPACE_1;

/// Gap between cards, both axes.
pub(crate) const CARD_GAP_PX: f32 = SPACE_3;

/// Rows shown before the gallery scrolls, at the narrow sizes.
///
/// Two, because one row cannot show that there are more — the same argument as
/// `MIN_ANCHORED_MENU_PX`'s two rows, and for the same reason: a single row of
/// cards reads as the whole set, so a user does not scroll and never learns what
/// else is there. Two rows with the second clipped mid-card is the affordance.
pub(crate) const COMPACT_VISIBLE_ROWS: f32 = 2.0;

/// How the gallery lays out at one size class.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct GalleryLayout {
    /// `Some(px)` caps the gallery's height and scrolls inside it; `None` lets
    /// it take the height its content needs.
    pub max_height_px: Option<f32>,
}

/// The gallery's layout for a measured size class.
///
/// # Wrapping with vertical scroll, never a horizontal scroller
///
/// The gallery was a `flex-direction: row` with `overflow-x: auto` and
/// `flex-shrink: 0` cards — **a horizontal-only scroll region**, which Phase 4's
/// acceptance forbids outright. Two reasons it is worse than it looks: a
/// horizontal scroller has no affordance on a touch device beyond guessing that
/// it scrolls, and on a desktop it is the axis a wheel does not move. Cards
/// beyond the first screenful were, in practice, undiscoverable.
///
/// So the gallery wraps and scrolls **vertically** at every size. What the size
/// class decides is only whether the height is *capped*:
///
/// | class | cap | why |
/// | --- | --- | --- |
/// | `Compact`, `Medium` | two rows | the gallery shares one narrow column with the recent list, and an uncapped grid would push the documents off the screen |
/// | `Expanded` | none | the gallery has the full left column, so its natural height is the right height and an internal scrollbar beside an unused one is noise |
///
/// `Medium` takes the capped form deliberately: the column is still shared
/// there, and the acceptance is about whether the recent list stays reachable,
/// not about a width in pixels.
#[must_use]
pub(crate) fn gallery_layout(breakpoint: Breakpoint) -> GalleryLayout {
    match breakpoint {
        // Written out rather than `_ =>` so a new size class is a compile error
        // here instead of silently joining whichever arm the wildcard covers.
        Breakpoint::Compact | Breakpoint::Medium => GalleryLayout {
            max_height_px: Some(compact_max_height()),
        },
        Breakpoint::Expanded => GalleryLayout {
            max_height_px: None,
        },
    }
}

/// The height of [`COMPACT_VISIBLE_ROWS`] rows, gaps included.
///
/// `rows * height + (rows - 1) * gap`, which for two rows is one gap. Derived
/// rather than written down so the cap follows the card.
#[must_use]
pub(crate) fn compact_max_height() -> f32 {
    COMPACT_VISIBLE_ROWS * CARD_HEIGHT_PX + (COMPACT_VISIBLE_ROWS - 1.0) * CARD_GAP_PX
}

#[cfg(test)]
#[path = "gallery_layout_tests.rs"]
mod tests;
