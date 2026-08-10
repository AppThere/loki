// SPDX-License-Identifier: Apache-2.0

//! Edge presets for the paragraph Borders tab.
//!
//! The model carries four independent edges. The dialog offers the three
//! configurations that account for essentially every paragraph border in a
//! document — none, a start-edge rule (the pull-quote), and a full box — and
//! keeps one shared style/width/colour across whichever edges are on. A
//! four-edge matrix with per-edge style is the LibreOffice behaviour this
//! deliberately does not copy: it costs a whole tab of controls to express
//! combinations nobody sets.
//!
//! An imported document may of course carry an edge set this cannot name; that
//! is [`BorderEdges::Mixed`], which is reported and left alone rather than
//! silently rounded to the nearest preset.

use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::props::border::{Border, BorderStyle};

/// Which edges carry a border.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BorderEdges {
    /// No edge is set.
    None,
    /// The start edge only — the pull-quote rule.
    StartOnly,
    /// All four edges — a box.
    All,
    /// Some other combination, from an imported document.
    Mixed,
}

impl BorderEdges {
    /// The presets the dialog offers, in display order. [`BorderEdges::Mixed`]
    /// is deliberately absent — it is a state to report, not one to choose.
    pub const SELECTABLE: [BorderEdges; 3] =
        [BorderEdges::None, BorderEdges::StartOnly, BorderEdges::All];

    /// Which preset `style` is currently in.
    #[must_use]
    pub fn of(style: &ParagraphStyle) -> Self {
        let p = &style.para_props;
        let set = [
            p.border_top.is_some(),
            p.border_bottom.is_some(),
            p.border_left.is_some(),
            p.border_right.is_some(),
        ];
        match set {
            [false, false, false, false] => BorderEdges::None,
            [true, true, true, true] => BorderEdges::All,
            // Start edge is `left` in an LTR paragraph; `bidi` flips it.
            [false, false, true, false] if !p.bidi.unwrap_or(false) => BorderEdges::StartOnly,
            [false, false, false, true] if p.bidi.unwrap_or(false) => BorderEdges::StartOnly,
            _ => BorderEdges::Mixed,
        }
    }

    /// Whether this preset is one the picker can select.
    #[must_use]
    pub fn is_selectable(self) -> bool {
        !matches!(self, BorderEdges::Mixed)
    }

    /// The index of this preset among [`Self::SELECTABLE`], if it is one.
    #[must_use]
    pub fn selectable_index(self) -> Option<usize> {
        BorderEdges::SELECTABLE.iter().position(|e| *e == self)
    }

    /// Writes this preset onto `style`, carrying `border` onto whichever edges
    /// the preset turns on.
    pub fn apply(self, style: &mut ParagraphStyle, border: Border) {
        let bidi = style.para_props.bidi.unwrap_or(false);
        let p = &mut style.para_props;
        let (top, bottom, left, right) = match self {
            BorderEdges::None => (None, None, None, None),
            BorderEdges::All => (
                Some(border.clone()),
                Some(border.clone()),
                Some(border.clone()),
                Some(border),
            ),
            BorderEdges::StartOnly if bidi => (None, None, None, Some(border)),
            BorderEdges::StartOnly => (None, None, Some(border), None),
            // Never write `Mixed` — the picker cannot select it, and reaching
            // here would mean turning an imported edge set into a guess.
            BorderEdges::Mixed => return,
        };
        p.border_top = top;
        p.border_bottom = bottom;
        p.border_left = left;
        p.border_right = right;
    }
}

/// The border the style's edges share, for seeding the style/width/colour
/// controls. Returns the start edge when present, else the first edge that is.
#[must_use]
pub(super) fn edge_border(style: &ParagraphStyle) -> Option<Border> {
    let p = &style.para_props;
    let start_first = if p.bidi.unwrap_or(false) {
        [
            &p.border_right,
            &p.border_left,
            &p.border_top,
            &p.border_bottom,
        ]
    } else {
        [
            &p.border_left,
            &p.border_right,
            &p.border_top,
            &p.border_bottom,
        ]
    };
    start_first.into_iter().flatten().next().cloned()
}

/// The border-line styles the dialog offers.
///
/// Groove, Ridge, Inset and Outset are omitted: they are bevel effects that
/// need two tones of a colour to read, which neither Blitz nor a 1-bit E-Ink
/// panel can paint — an effect that silently disappears is worse than one that
/// was never offered (the same call the design makes for outline/emboss/shadow).
pub(super) const BORDER_STYLES: [BorderStyle; 5] = [
    BorderStyle::Solid,
    BorderStyle::Dashed,
    BorderStyle::Dotted,
    BorderStyle::Double,
    BorderStyle::Wave,
];

#[cfg(test)]
#[path = "borders_tests.rs"]
mod tests;
