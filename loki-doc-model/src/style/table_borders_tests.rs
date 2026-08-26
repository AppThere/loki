// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super`] — the six-sided table border set, its per-cell edge
//! resolution, and how a table's own set layers over its style's.

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

/// The rule measured against Word in `table-direct-borders.docx`: a table's
/// own `w:tblBorders` merges with the style's **per edge**, and an edge
/// explicitly `w:val="none"` is a *statement* (suppress), not a silence
/// (fall back).
#[test]
fn direct_borders_layer_over_the_style_set_per_edge() {
    let solid = |w: f64| {
        Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(w),
            color: None,
            spacing: None,
        })
    };
    let nil = Some(Border {
        style: BorderStyle::None,
        width: Points::new(0.0),
        color: None,
        spacing: None,
    });
    // A full-grid style, every edge thin — the probe's `ProbeGrid`.
    let style = TableBorders {
        top: solid(0.5),
        left: solid(0.5),
        bottom: solid(0.5),
        right: solid(0.5),
        inside_h: solid(0.5),
        inside_v: solid(0.5),
    };
    let w = |e: &Option<Border>| e.as_ref().map(|x| x.width.value());
    let drawn = |e: &Option<Border>| e.as_ref().is_some_and(|x| x.style != BorderStyle::None);

    // Probe table B: outer only, interior edges ABSENT.
    let outer_only = TableBorders {
        top: solid(3.0),
        left: solid(3.0),
        bottom: solid(3.0),
        right: solid(3.0),
        ..TableBorders::default()
    };
    let b = outer_only.over(&style);
    // The direct outer edges win...
    assert_eq!(w(&b.top), Some(3.0));
    assert_eq!(w(&b.right), Some(3.0));
    // ...and the unstated interior edges fall back to the style's thin
    // gridlines. Word draws them; wholesale replacement would not.
    assert_eq!(w(&b.inside_h), Some(0.5), "absent edge must fall back");
    assert_eq!(w(&b.inside_v), Some(0.5), "absent edge must fall back");
    assert!(drawn(&b.inside_h));

    // Probe table C: same outer edges, interior edges explicitly `none`.
    // Identical to B except for that one distinction, so any difference in
    // the result is attributable to it alone.
    let outer_plus_none = TableBorders {
        inside_h: nil.clone(),
        inside_v: nil.clone(),
        ..outer_only.clone()
    };
    let c = outer_plus_none.over(&style);
    assert_eq!(w(&c.top), Some(3.0), "outer must still win");
    // Inverted predicate: the edge is *present* (it was stated) but must
    // draw nothing. A mapper that dropped explicit-`none` to `None` would
    // give `Some(0.5)` here and silently reinstate the style's gridlines.
    assert!(c.inside_h.is_some(), "explicit none is a statement");
    assert!(!drawn(&c.inside_h), "explicit none must suppress");
    assert!(!drawn(&c.inside_v), "explicit none must suppress");

    // Guard inversion: `over` must be a no-op in both degenerate
    // directions, or "per-edge merge" is really "whichever side is
    // non-empty wins".
    assert_eq!(TableBorders::default().over(&style), style);
    assert_eq!(outer_only.over(&TableBorders::default()), outer_only);

    // A set of only explicit-`none` edges draws nothing but is not empty —
    // it still has something to say when layered over a grid.
    let all_none = TableBorders {
        top: nil.clone(),
        left: nil.clone(),
        bottom: nil.clone(),
        right: nil.clone(),
        inside_h: nil.clone(),
        inside_v: nil,
    };
    assert!(!all_none.is_empty());
    assert!(!drawn(&all_none.over(&style).inside_h));
}

#[test]
fn resolve_cell_borders_contributes_nothing_without_a_border_set() {
    // Guard inversion: an absent set and a present-but-empty set must both
    // contribute nothing. The second case is the one that matters — a
    // `Some(TableBorders::default())` reaching here means some style in the
    // chain was found but specified no edges, and it must not read as
    // "borders exist" merely because the `Option` is `Some`.
    assert_eq!(resolve_cell_borders(None, 0, 0, 2, 2), CellEdges::default());
    assert_eq!(
        resolve_cell_borders(Some(&TableBorders::default()), 0, 0, 2, 2),
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
