// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The named **paper-size catalogue** (Spec 08 T6.2): ISO A/B/C series, JIS B
//! series, the US sizes, envelopes and index cards.
//!
//! # A name is derived, never stored
//!
//! Neither format carries a paper *name*. ODF writes `fo:page-width` /
//! `fo:page-height`, OOXML writes `w:pgSz/@w:w` and `@w:h` — dimensions only.
//! (OOXML's `w:pgSz/@w:code` is a Windows `DEVMODE` printer paper code, which
//! Loki neither reads nor writes.) So a [`Paper`] is **looked up from the
//! dimensions**, and [`PageSize`] gains no name field: there is one fact — the
//! width and height — and one derivation from it. A page whose size matches no
//! entry is a user-defined size and simply has no name; that is a normal state,
//! not an error.
//!
//! # The matching rule lives here
//!
//! [`Paper::matches`] is the single definition of "this page is that paper":
//! orientation-independent (compare short and long edges), within
//! [`MATCH_TOLERANCE_PT`]. Three copies of this rule previously lived in the UI
//! — the style inspector, the page-style form's active-preset check, and the
//! Layout ribbon — each with its own literal dimensions.

use crate::layout::page::PageSize;

/// How far a page's edges may differ from a catalogued paper and still be
/// called by its name, in points.
///
/// One point, because the catalogue is stated in points converted from mm/inch
/// and a document's stored size is a rounded conversion of the same.
///
/// [`Paper::matches`] requires **both** axes to be within tolerance, so what
/// bounds this constant is each pair's *better*-separated axis, minimised over
/// the catalogue — not the closest single edge. Several pairs share one edge
/// exactly (Folio and Legal are both 612 pt wide) and are still never
/// confusable. `paper_entries_are_mutually_distinguishable` computes that bound
/// and asserts this tolerance stays under it, so no page can be named by two
/// papers and `paper_for`'s first-match answer cannot depend on row order.
pub const MATCH_TOLERANCE_PT: f64 = 1.0;

/// A catalogued paper size, stated in **portrait** orientation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
    /// Stable machine key (kebab-case), e.g. `"a4"`, `"us-letter"`. Used to
    /// address a paper from settings and the UI; never shown to the user.
    pub id: &'static str,
    /// The name shown in the UI, e.g. `"A4"`, `"US Letter"`.
    ///
    /// A named constant rather than a hardcoded literal at the point of use
    /// (the `loki_i18n` convention). Paper names are near-universally
    /// untranslated proper nouns; the handful that are not (`"US Letter"`,
    /// `"Legal"`, `"Executive"`, `"Statement"`, the index cards) would need
    /// per-entry Fluent keys — see `TODO(paper-i18n)`.
    pub display_name: &'static str,
    /// Portrait width in points.
    pub width_pt: f64,
    /// Portrait height in points.
    pub height_pt: f64,
}

impl Paper {
    /// This paper as a portrait [`PageSize`].
    #[must_use]
    pub fn portrait(&self) -> PageSize {
        PageSize {
            width: crate::loki_primitives::units::Points::new(self.width_pt),
            height: crate::loki_primitives::units::Points::new(self.height_pt),
        }
    }

    /// This paper in the orientation of `like` — landscape when that page is
    /// wider than tall. Choosing a new paper must not silently rotate the page.
    #[must_use]
    pub fn oriented_like(&self, like: &PageSize) -> PageSize {
        let p = self.portrait();
        if like.width.value() > like.height.value() {
            PageSize {
                width: p.height,
                height: p.width,
            }
        } else {
            p
        }
    }

    /// Whether `size` is this paper, comparing short and long edges within
    /// [`MATCH_TOLERANCE_PT`] — so a landscape A4 page is still A4.
    #[must_use]
    pub fn matches(&self, size: &PageSize) -> bool {
        let (w, h) = (size.width.value(), size.height.value());
        let (short, long) = (w.min(h), w.max(h));
        let (pshort, plong) = (
            self.width_pt.min(self.height_pt),
            self.width_pt.max(self.height_pt),
        );
        (short - pshort).abs() < MATCH_TOLERANCE_PT && (long - plong).abs() < MATCH_TOLERANCE_PT
    }
}

/// Builds a catalogue entry from millimetres (ISO/JIS papers are defined in mm).
const fn mm(id: &'static str, display_name: &'static str, w: f64, h: f64) -> Paper {
    Paper {
        id,
        display_name,
        width_pt: w * 72.0 / 25.4,
        height_pt: h * 72.0 / 25.4,
    }
}

/// Builds a catalogue entry from inches (the US papers are defined in inches).
const fn inch(id: &'static str, display_name: &'static str, w: f64, h: f64) -> Paper {
    Paper {
        id,
        display_name,
        width_pt: w * 72.0,
        height_pt: h * 72.0,
    }
}

/// ISO 216 A4 — the metric default, named here so [`PageSize::a4`] and the
/// catalogue cannot disagree about its dimensions.
pub const A4: Paper = mm("a4", "A4", 210.0, 297.0);

/// US Letter — the imperial default, and [`PageSize::letter`]'s definition.
pub const US_LETTER: Paper = inch("us-letter", "US Letter", 8.5, 11.0);

/// Every catalogued paper, in the order the UI lists them: ISO A, ISO B, JIS B,
/// ISO C envelopes, US sizes, US envelopes, index cards.
pub const PAPERS: &[Paper] = &[
    // ISO 216 A series.
    mm("a0", "A0", 841.0, 1189.0),
    mm("a1", "A1", 594.0, 841.0),
    mm("a2", "A2", 420.0, 594.0),
    mm("a3", "A3", 297.0, 420.0),
    A4,
    mm("a5", "A5", 148.0, 210.0),
    mm("a6", "A6", 105.0, 148.0),
    // ISO 216 B series.
    mm("iso-b4", "ISO B4", 250.0, 353.0),
    mm("iso-b5", "ISO B5", 176.0, 250.0),
    mm("iso-b6", "ISO B6", 125.0, 176.0),
    // JIS B series — deliberately distinct ids from the ISO B entries: they
    // share the "B5" label in the wild and differ by ~17 pt, which is well
    // outside the match tolerance, so a document is never ambiguously both.
    mm("jis-b4", "JIS B4", 257.0, 364.0),
    mm("jis-b5", "JIS B5", 182.0, 257.0),
    mm("jis-b6", "JIS B6", 128.0, 182.0),
    // ISO 269 C series envelopes.
    mm("c5", "C5 envelope", 162.0, 229.0),
    mm("c6", "C6 envelope", 114.0, 162.0),
    mm("dl", "DL envelope", 110.0, 220.0),
    // US sizes.
    US_LETTER,
    inch("us-legal", "Legal", 8.5, 14.0),
    inch("tabloid", "Tabloid", 11.0, 17.0),
    inch("executive", "Executive", 7.25, 10.5),
    inch("statement", "Statement", 5.5, 8.5),
    inch("folio", "Folio", 8.5, 13.0),
    mm("quarto", "Quarto", 215.0, 275.0),
    // US envelopes.
    inch("env-10", "Envelope #10", 4.125, 9.5),
    inch("env-monarch", "Envelope Monarch", 3.875, 7.5),
    // Index cards.
    inch("index-3x5", "Index card 3 × 5", 3.0, 5.0),
    inch("index-4x6", "Index card 4 × 6", 4.0, 6.0),
    inch("index-5x8", "Index card 5 × 8", 5.0, 8.0),
];

/// The catalogued paper `size` is, or `None` for a user-defined size.
///
/// Orientation-independent — a landscape A4 page returns A4.
#[must_use]
pub fn paper_for(size: &PageSize) -> Option<&'static Paper> {
    PAPERS.iter().find(|p| p.matches(size))
}

/// The catalogued paper with this stable id, or `None`.
#[must_use]
pub fn paper_by_id(id: &str) -> Option<&'static Paper> {
    PAPERS.iter().find(|p| p.id == id)
}

#[cfg(test)]
#[path = "paper_catalog_tests.rs"]
mod tests;
