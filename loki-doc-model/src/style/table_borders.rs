// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The six-sided table border set and its per-cell edge resolution.

use crate::style::props::border::Border;

/// The six-sided border set of a table (`w:tblBorders`, ECMA-376 §17.4.39):
/// the four outer edges plus the interior gridlines applied *between* cells.
///
/// A cell's four effective edges are picked by position — an outer edge on the
/// table boundary, otherwise the matching interior gridline — by
/// [`edges_for`](TableBorders::edges_for). This is how a *Table Grid* style
/// (every side a single hairline) paints a full grid.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableBorders {
    pub top: Option<Border>,
    pub left: Option<Border>,
    pub bottom: Option<Border>,
    pub right: Option<Border>,
    /// Interior horizontal gridline, drawn between vertically-adjacent cells.
    pub inside_h: Option<Border>,
    /// Interior vertical gridline, drawn between horizontally-adjacent cells.
    pub inside_v: Option<Border>,
}

/// The four effective borders of one cell: `(top, right, bottom, left)`.
pub type CellEdges = (
    Option<Border>,
    Option<Border>,
    Option<Border>,
    Option<Border>,
);

impl TableBorders {
    /// The `(top, right, bottom, left)` borders for the cell at `(row, col)` in a
    /// `rows`×`cols` grid: an outer edge on the table boundary, otherwise the
    /// interior gridline for that axis.
    #[must_use]
    pub fn edges_for(&self, row: usize, col: usize, rows: usize, cols: usize) -> CellEdges {
        let top = if row == 0 {
            self.top.clone()
        } else {
            self.inside_h.clone()
        };
        let bottom = if row + 1 >= rows {
            self.bottom.clone()
        } else {
            self.inside_h.clone()
        };
        let left = if col == 0 {
            self.left.clone()
        } else {
            self.inside_v.clone()
        };
        let right = if col + 1 >= cols {
            self.right.clone()
        } else {
            self.inside_v.clone()
        };
        (top, right, bottom, left)
    }

    /// This set layered **over** `base`: `self`'s stated edges win, and each
    /// edge `self` leaves unstated falls back to `base`'s.
    ///
    /// This is how a table's own `w:tblPr/w:tblBorders` resolves against the
    /// set contributed by the style it references. The merge is **per edge**,
    /// not wholesale: measured against Word (`table-direct-borders.docx`), a
    /// table on a full-grid style whose direct set gives only the four outer
    /// edges still draws the style's interior gridlines — thin, from the style
    /// — inside its own thick outer frame.
    ///
    /// The distinction between an edge that is *absent* and one explicitly
    /// `w:val="none"` is load-bearing here and must survive into `self`:
    /// absent falls back, explicit-none suppresses. That is why an explicit
    /// `none` is carried as `Some(Border { style: None, .. })` rather than
    /// dropped — the same fixture's third table pins it.
    #[must_use]
    pub fn over(&self, base: &TableBorders) -> TableBorders {
        let pick = |own: &Option<Border>, under: &Option<Border>| own.clone().or(under.clone());
        TableBorders {
            top: pick(&self.top, &base.top),
            left: pick(&self.left, &base.left),
            bottom: pick(&self.bottom, &base.bottom),
            right: pick(&self.right, &base.right),
            inside_h: pick(&self.inside_h, &base.inside_h),
            inside_v: pick(&self.inside_v, &base.inside_v),
        }
    }

    /// `true` when every edge is absent.
    ///
    /// An edge explicitly set to `w:val="none"` is *stated* — it suppresses
    /// whatever it is layered over — so a set consisting only of such edges is
    /// **not** empty, even though it draws nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.top.is_none()
            && self.left.is_none()
            && self.bottom.is_none()
            && self.right.is_none()
            && self.inside_h.is_none()
            && self.inside_v.is_none()
    }
}

/// The `(top, right, bottom, left)` borders a table *style* contributes to the
/// cell at `(row, col)`, or all-`None` when the style sets no borders.
///
/// The style→cell half of a cell's border resolution. The other half — a
/// direct `CellProps` border winning per edge — belongs to the caller, which
/// must `or` each edge over these; see [`effective_cell_edges`].
///
/// `borders` is the set already resolved through the style's `basedOn` chain
/// (see [`StyleCatalog::table_borders_for`](crate::style::StyleCatalog::table_borders_for)),
/// **not** a style to read them off. Taking a `&TableStyle` here is what let
/// callers do a flat `catalog.table_styles[name]` lookup and silently miss
/// borders that the style inherits from its parent — the signature now makes
/// that call impossible to write.
#[must_use]
pub fn resolve_cell_borders(
    borders: Option<&TableBorders>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> CellEdges {
    borders
        .map(|b| b.edges_for(row, col, rows, cols))
        .unwrap_or_default()
}

/// A cell's four effective borders: a direct `CellProps` edge wins, else the
/// table style's edge for that position.
///
/// The resolution is **per edge** — a cell with only a direct top border still
/// takes its other three from the style. Both the paint path and ODT export
/// call this, so a table cannot draw one grid on screen and export another:
/// exporting borders resolved by a different rule than the renderer uses is
/// how a document silently changes appearance on save.
#[must_use]
pub fn effective_cell_edges(
    direct: (
        Option<&Border>,
        Option<&Border>,
        Option<&Border>,
        Option<&Border>,
    ),
    from_style: &CellEdges,
) -> CellEdges {
    (
        direct.0.cloned().or_else(|| from_style.0.clone()),
        direct.1.cloned().or_else(|| from_style.1.clone()),
        direct.2.cloned().or_else(|| from_style.2.clone()),
        direct.3.cloned().or_else(|| from_style.3.clone()),
    )
}

#[cfg(test)]
#[path = "table_borders_tests.rs"]
mod tests;
