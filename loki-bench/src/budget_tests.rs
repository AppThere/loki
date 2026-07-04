// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-tier peak-RSS budget checks + budgets-file round-trip (Spec 06 M5).

use super::*;

#[test]
fn within_and_over_budget() {
    assert_eq!(check(500, 1_000), BudgetStatus::WithinBudget);
    assert_eq!(check(1_000, 1_000), BudgetStatus::WithinBudget); // boundary is within
    assert_eq!(check(1_001, 1_000), BudgetStatus::OverBudget);
}

#[test]
fn headroom_is_signed_fraction_of_budget() {
    assert!((headroom_frac(500, 1_000) - 0.5).abs() < 1e-9);
    assert!(headroom_frac(1_500, 1_000) < 0.0); // over budget → negative
    assert_eq!(headroom_frac(10, 0), 0.0);
}

#[test]
fn budgets_render_then_parse_round_trips() {
    let b = Budgets::from_pairs(&[
        ("large".to_string(), 900_000_000),
        ("small".to_string(), 300_000_000),
    ]);
    assert_eq!(Budgets::parse(&b.render()).expect("parse"), b);
    assert_eq!(b.get("large"), Some(900_000_000));
    assert_eq!(b.get("missing"), None);
}

#[test]
fn budgets_parse_rejects_malformed_lines() {
    assert!(Budgets::parse("small 100 extra\n").is_err()); // 3 fields
    assert!(Budgets::parse("small notanumber\n").is_err());
}
