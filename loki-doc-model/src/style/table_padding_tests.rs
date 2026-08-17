// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`CellPadding`] and [`effective_cell_padding`].

use super::{CellPadding, effective_cell_padding};
use loki_primitives::units::Points;

fn v(p: Option<Points>) -> Option<f64> {
    p.map(|x| x.value())
}

#[test]
fn effective_padding_resolves_per_side_not_all_or_nothing() {
    // Word's own default shape: asymmetric, and the vertical sides are
    // deliberately absent rather than zero.
    let from_style = CellPadding {
        top: None,
        bottom: None,
        left: Some(Points::new(5.4)),
        right: Some(Points::new(5.4)),
    };

    // A cell setting only `top` must still inherit left/right from the style.
    // An all-or-nothing rule returns (9, None, None, None) here and fails.
    let eff = effective_cell_padding((Some(Points::new(9.0)), None, None, None), &from_style);
    assert_eq!(
        (v(eff.0), v(eff.1), v(eff.2), v(eff.3)),
        (Some(9.0), None, Some(5.4), Some(5.4))
    );

    // A direct side overrides only itself, even when the style sets it too.
    let eff = effective_cell_padding((None, None, Some(Points::new(1.0)), None), &from_style);
    assert_eq!(
        (v(eff.0), v(eff.1), v(eff.2), v(eff.3)),
        (None, None, Some(1.0), Some(5.4))
    );
}

#[test]
fn a_style_with_no_padding_contributes_nothing() {
    // Guard inversion: with an empty style contribution the direct values must
    // pass through untouched, and absent sides must stay absent rather than
    // becoming zero — a cell with no padding is not a cell with 0pt padding
    // for the purposes of further inheritance.
    let eff = effective_cell_padding(
        (Some(Points::new(2.0)), None, None, None),
        &CellPadding::default(),
    );
    assert_eq!(
        (v(eff.0), v(eff.1), v(eff.2), v(eff.3)),
        (Some(2.0), None, None, None)
    );
}

#[test]
fn an_explicit_zero_side_is_not_the_same_as_an_absent_one() {
    // `w:tblCellMar` routinely sets top/bottom to an explicit 0. That must win
    // over a style default of 5.4, so `Some(0.0)` cannot be treated as "unset".
    let from_style = CellPadding {
        top: Some(Points::new(5.4)),
        bottom: Some(Points::new(5.4)),
        left: Some(Points::new(5.4)),
        right: Some(Points::new(5.4)),
    };
    let eff = effective_cell_padding((Some(Points::new(0.0)), None, None, None), &from_style);
    assert_eq!(
        v(eff.0),
        Some(0.0),
        "an explicit 0 must not fall back to 5.4"
    );
}

#[test]
fn is_empty_distinguishes_unset_from_zero() {
    assert!(CellPadding::default().is_empty());
    assert!(
        !CellPadding {
            top: Some(Points::new(0.0)),
            ..Default::default()
        }
        .is_empty(),
        "an explicit 0 side is a value, not an absence"
    );
}
