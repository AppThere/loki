// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Baseline round-trip + diff classification (Spec 06 M3). The headline check is
//! that a deliberate regression — including a shared resource cloned by value
//! instead of by `Arc` (a near-zero metric turning large) — is flagged
//! `Regressed`.

use super::*;

fn stats(bytes: u64, blocks: u64) -> AllocStats {
    AllocStats {
        total_bytes: bytes,
        total_blocks: blocks,
        max_bytes: bytes,
        max_blocks: blocks,
    }
}

#[test]
fn render_then_parse_round_trips() {
    let base = Baseline::from_samples(&[
        ("layout/medium".to_string(), stats(9_737_350, 14_325)),
        ("arc/share_font_resources".to_string(), stats(0, 0)),
    ]);
    let parsed = Baseline::parse(&base.render()).expect("parse");
    assert_eq!(parsed, base);
}

#[test]
fn parse_ignores_comments_and_blank_lines() {
    let text = "# header\n\n  # indented comment\nlayout/small 100 5 80 4\n";
    let b = Baseline::parse(text).expect("parse");
    assert_eq!(b.len(), 1);
    assert_eq!(b.get("layout/small").map(|s| s.total_blocks), Some(5));
}

#[test]
fn parse_rejects_malformed_lines() {
    assert!(Baseline::parse("key 1 2 3\n").is_err()); // 4 fields, need 5
    assert!(Baseline::parse("key x 2 3 4\n").is_err()); // non-integer
}

#[test]
fn a_deliberate_growth_regression_is_flagged() {
    let base = Baseline::from_samples(&[("layout/medium".to_string(), stats(1_000_000, 10_000))]);
    // Same key now allocates ~2x — a real regression, far past tolerance.
    let current = vec![("layout/medium".to_string(), stats(2_000_000, 20_000))];
    let deltas = diff(&current, &base, Tolerance::default());
    assert_eq!(deltas[0].status, DeltaStatus::Regressed);
    assert!(any_regressed(&deltas));
}

#[test]
fn arc_share_replaced_by_value_clone_is_flagged() {
    // The acceptance case: the shared-Arc metric is 0 allocations in the
    // baseline; a value-clone regression makes it allocate, which must flag.
    let base = Baseline::from_samples(&[("arc/share_font_resources".to_string(), stats(0, 0))]);
    let current = vec![("arc/share_font_resources".to_string(), stats(20_000_000, 1))];
    let deltas = diff(&current, &base, Tolerance::default());
    assert_eq!(deltas[0].status, DeltaStatus::Regressed);
    assert!(!deltas[0].bytes_pct.is_finite(), "0 -> nonzero is +INF");
}

#[test]
fn within_tolerance_jitter_is_unchanged() {
    // DOCX byte drift of a few dozen bytes on ~2 MB is well within tolerance.
    let base = Baseline::from_samples(&[("io/medium_save".to_string(), stats(2_186_331, 720))]);
    let current = vec![("io/medium_save".to_string(), stats(2_186_362, 720))];
    let deltas = diff(&current, &base, Tolerance::default());
    assert_eq!(deltas[0].status, DeltaStatus::Unchanged);
}

#[test]
fn improvement_and_new_and_removed_are_classified() {
    let base = Baseline::from_samples(&[
        ("keep".to_string(), stats(1_000_000, 10_000)),
        ("gone".to_string(), stats(500, 5)),
    ]);
    let current = vec![
        ("keep".to_string(), stats(500_000, 5_000)), // halved → improved
        ("fresh".to_string(), stats(100, 2)),        // not in baseline → new
    ];
    let deltas = diff(&current, &base, Tolerance::default());
    let by = |k: &str| deltas.iter().find(|d| d.key == k).map(|d| d.status);
    assert_eq!(by("keep"), Some(DeltaStatus::Improved));
    assert_eq!(by("fresh"), Some(DeltaStatus::New));
    assert_eq!(by("gone"), Some(DeltaStatus::Removed));
}
