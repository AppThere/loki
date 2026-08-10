// SPDX-License-Identifier: Apache-2.0

//! Tests for the tab-stop list's materialisation.

use super::*;
use loki_doc_model::loki_primitives::units::Points;

fn at(pts: f64) -> TabStop {
    TabStop::left(Points::new(pts))
}

fn positions(stops: &[TabStop]) -> Vec<f64> {
    stops.iter().map(|s| s.position.value()).collect()
}

/// The list the table shows is the base for every edit. A style that inherits
/// its stops has no local list, so starting from the local one would turn
/// "delete the middle stop" into "delete all three".
#[test]
fn deleting_one_inherited_stop_keeps_the_others() {
    let inherited = vec![at(56.0), at(112.0), at(168.0)];

    let after = edited_stops(&inherited, |list| {
        list.remove(1);
    });

    assert_eq!(positions(&after), vec![56.0, 168.0]);
}

/// Same root cause from the other direction: adding to an inherited set must
/// keep the set, not replace it with the one new stop.
#[test]
fn adding_to_an_inherited_set_keeps_the_inherited_stops() {
    let inherited = vec![at(56.0), at(112.0)];

    let after = edited_stops(&inherited, |list| list.push(at(224.0)));

    assert_eq!(positions(&after), vec![56.0, 112.0, 224.0]);
}

/// The engine walks stops in order, so an out-of-order insert is sorted back.
#[test]
fn an_out_of_order_insert_is_sorted_into_place() {
    let after = edited_stops(&[at(56.0), at(168.0)], |list| list.push(at(112.0)));

    assert_eq!(positions(&after), vec![56.0, 112.0, 168.0]);
}

/// Clear passes an empty base deliberately — the one case where starting from
/// nothing is the intent, and the result must be an explicit empty local list
/// rather than a fall-through to the parent.
#[test]
fn clearing_produces_an_empty_local_list() {
    let after = edited_stops(&[], |list| list.clear());

    assert!(after.is_empty());
}
