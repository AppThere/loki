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

    /// `true` when every edge is absent.
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
#[must_use]
pub fn resolve_cell_borders(
    style: Option<&crate::style::table_style::TableStyle>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> CellEdges {
    style
        .and_then(|s| s.table_props.borders.as_ref())
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
mod tests {
    use super::*;
    use crate::style::props::border::BorderStyle;
    use loki_primitives::units::Points;

    #[test]
    fn tbl_borders_edges_pick_outer_vs_interior() {
        // A "Table Grid"-like set: distinct markers per edge so we can tell which
        // one each cell position resolves to.
        let mk = |w: f64| {
            Some(Border {
                style: BorderStyle::Solid,
                width: Points::new(w),
                color: None,
                spacing: None,
            })
        };
        let b = TableBorders {
            top: mk(1.0),
            left: mk(2.0),
            bottom: mk(3.0),
            right: mk(4.0),
            inside_h: mk(5.0),
            inside_v: mk(6.0),
        };
        let w = |e: &Option<Border>| e.as_ref().map(|x| x.width.value());

        // Top-left cell of a 3×3 grid: outer top+left, interior bottom+right.
        let (t, r, bo, l) = b.edges_for(0, 0, 3, 3);
        assert_eq!(
            (w(&t), w(&r), w(&bo), w(&l)),
            (Some(1.0), Some(6.0), Some(5.0), Some(2.0))
        );

        // Centre cell: interior on all four sides.
        let (t, r, bo, l) = b.edges_for(1, 1, 3, 3);
        assert_eq!(
            (w(&t), w(&r), w(&bo), w(&l)),
            (Some(5.0), Some(6.0), Some(5.0), Some(6.0))
        );

        // Bottom-right cell: interior top+left, outer bottom+right.
        let (t, r, bo, l) = b.edges_for(2, 2, 3, 3);
        assert_eq!(
            (w(&t), w(&r), w(&bo), w(&l)),
            (Some(5.0), Some(4.0), Some(3.0), Some(6.0))
        );

        assert!(!b.is_empty());
        assert!(TableBorders::default().is_empty());
    }

    #[test]
    fn resolve_cell_borders_needs_both_a_style_and_a_border_set() {
        use crate::style::table_style::{TableProps, TableStyle};
        let plain = TableStyle {
            id: crate::style::catalog::StyleId::new("Plain"),
            display_name: None,
            parent: None,
            table_props: TableProps::default(),
            conditional: Default::default(),
            extensions: Default::default(),
        };
        // Guard inversion: no style at all, and a style with no border set,
        // must both contribute nothing — otherwise "has a style" would be
        // standing in for "has borders".
        assert_eq!(resolve_cell_borders(None, 0, 0, 2, 2), CellEdges::default());
        assert_eq!(
            resolve_cell_borders(Some(&plain), 0, 0, 2, 2),
            CellEdges::default()
        );
    }

    #[test]
    fn effective_cell_edges_resolves_per_edge_not_all_or_nothing() {
        let mk = |w: f64| Border {
            style: BorderStyle::Solid,
            width: Points::new(w),
            color: None,
            spacing: None,
        };
        let from_style = (Some(mk(1.0)), Some(mk(2.0)), Some(mk(3.0)), Some(mk(4.0)));
        let own_top = mk(9.0);
        let w = |e: &Option<Border>| e.as_ref().map(|x| x.width.value());

        // One direct edge must override *only* that edge. An all-or-nothing
        // rule returns (9, None, None, None) here and fails.
        let eff = effective_cell_edges((Some(&own_top), None, None, None), &from_style);
        assert_eq!(
            (w(&eff.0), w(&eff.1), w(&eff.2), w(&eff.3)),
            (Some(9.0), Some(2.0), Some(3.0), Some(4.0))
        );

        // No direct edges: the style's set passes through unchanged.
        let eff = effective_cell_edges((None, None, None, None), &from_style);
        assert_eq!(
            (w(&eff.0), w(&eff.1), w(&eff.2), w(&eff.3)),
            (Some(1.0), Some(2.0), Some(3.0), Some(4.0))
        );

        // No style contribution: the direct edge stands alone, and the absent
        // ones stay absent rather than inventing a border.
        let eff = effective_cell_edges((Some(&own_top), None, None, None), &CellEdges::default());
        assert_eq!(
            (w(&eff.0), w(&eff.1), w(&eff.2), w(&eff.3)),
            (Some(9.0), None, None, None)
        );
    }
}
