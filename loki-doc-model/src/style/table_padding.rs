// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A table style's default cell padding and its per-cell resolution.

use loki_primitives::units::Points;

/// The default cell padding a table style applies to every cell inside it
/// (`w:tblCellMar`, ECMA-376 §17.4.43).
///
/// Four independent sides, because Word's own default is asymmetric —
/// `top`/`bottom` `0`, `left`/`right` `108` twips (5.4pt) — so a single scalar
/// cannot represent it without inventing vertical padding that the document
/// does not ask for.
///
/// Unlike [`TableBorders`](crate::style::table_borders::TableBorders), this does
/// **not** vary by cell position: `w:tblCellMar` is one value for the whole
/// table. Resolution against a cell's direct padding is therefore
/// position-free — see [`effective_cell_padding`].
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellPadding {
    pub top: Option<Points>,
    pub bottom: Option<Points>,
    pub left: Option<Points>,
    pub right: Option<Points>,
}

impl CellPadding {
    /// `true` when every side is absent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.bottom.is_none() && self.left.is_none() && self.right.is_none()
    }
}

/// A cell's four effective padding values: a direct `CellProps` padding wins,
/// else the table style's default for that side.
///
/// The resolution is **per side** — a cell that sets only `padding_top` still
/// takes the other three from the style. Layout and both exporters call this,
/// so a table cannot inset its text one way on screen and another way on save.
///
/// Returns `(top, bottom, left, right)` — matching the declaration order of the
/// `padding_*` fields on `CellProps`, not the clockwise order used by
/// [`CellEdges`](crate::style::table_borders::CellEdges). The two orders differ
/// deliberately: they are different types and mixing them up should not
/// silently typecheck.
#[must_use]
pub fn effective_cell_padding(
    direct: (
        Option<Points>,
        Option<Points>,
        Option<Points>,
        Option<Points>,
    ),
    from_style: &CellPadding,
) -> (
    Option<Points>,
    Option<Points>,
    Option<Points>,
    Option<Points>,
) {
    (
        direct.0.or(from_style.top),
        direct.1.or(from_style.bottom),
        direct.2.or(from_style.left),
        direct.3.or(from_style.right),
    )
}

#[cfg(test)]
#[path = "table_padding_tests.rs"]
mod tests;
