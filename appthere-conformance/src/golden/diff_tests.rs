// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use image::Rgba;

fn solid(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(c))
}

/// A permissive tolerance for structural tests (thresholds themselves are
/// calibrated separately — D5).
fn tol() -> Tolerance {
    Tolerance {
        min_ssim: 0.9,
        max_delta_e: 4.0,
    }
}

// ── SSIM core (promoted from loki-acid) ───────────────────────────────────────

#[test]
fn identical_images_pass_with_ssim_one() {
    let img = solid(128, 128, [120, 130, 140, 255]);
    let report = compare_pages(&img, &img, tol()).unwrap();
    assert!(report.passed);
    let worst = report.worst.unwrap();
    assert!((worst.ssim - 1.0).abs() < 1e-9, "ssim={}", worst.ssim);
    assert!(worst.delta_e < 1e-9, "delta_e={}", worst.delta_e);
}

#[test]
fn black_vs_white_fails_hard() {
    let a = solid(128, 128, [0, 0, 0, 255]);
    let b = solid(128, 128, [255, 255, 255, 255]);
    let report = compare_pages(&a, &b, tol()).unwrap();
    assert!(!report.passed);
    assert!(report.worst.unwrap().ssim < 0.05);
}

#[test]
fn dimension_mismatch_errors() {
    let a = solid(64, 64, [0, 0, 0, 255]);
    let b = solid(65, 64, [0, 0, 0, 255]);
    assert!(compare_pages(&a, &b, tol()).is_err());
}

// ── Worst-region semantics (the B-4 fix) ──────────────────────────────────────

/// A small localized defect must fail the page even though the page-wide
/// *mean* would comfortably pass — the worst region drives the result.
#[test]
fn localized_defect_is_not_averaged_away() {
    let golden = solid(256, 256, [255, 255, 255, 255]);
    let mut candidate = golden.clone();
    for y in 32..48 {
        for x in 32..48 {
            candidate.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let report = compare_pages(&golden, &candidate, tol()).unwrap();
    assert!(!report.passed, "a 16px black square must fail its region");
    // The defect lies in region (0, 0) (64px tiles).
    let worst = report.worst.unwrap();
    assert_eq!(worst.region, (0, 0), "worst region must be the defect tile");
    // Every *other* region is perfect — proving the mean would have passed.
    let clean: Vec<_> = report
        .regions
        .iter()
        .filter(|r| r.region != (0, 0))
        .collect();
    assert!(clean.iter().all(|r| (r.ssim - 1.0).abs() < 1e-9));
    let mean: f64 =
        report.regions.iter().map(|r| r.ssim).sum::<f64>() / report.regions.len() as f64;
    assert!(
        mean > tol().min_ssim,
        "the page-wide mean ({mean}) would have masked the defect"
    );
}

/// A colour shift with intact structure is caught by ΔE, not SSIM: same
/// geometry, hue-shifted page.
#[test]
fn colour_shift_is_caught_by_delta_e() {
    let golden = solid(128, 128, [200, 60, 60, 255]);
    let candidate = solid(128, 128, [60, 200, 60, 255]);
    let report = compare_pages(&golden, &candidate, tol()).unwrap();
    assert!(!report.passed, "a hue swap must fail on ΔE");
    let worst = report.worst.unwrap();
    assert!(
        worst.delta_e > tol().max_delta_e,
        "delta_e={} must exceed the bound",
        worst.delta_e
    );
}

// ── CIEDE2000 reference values (Sharma et al. 2005 test data) ─────────────────

#[test]
fn ciede2000_matches_reference_pairs() {
    // (Lab1, Lab2, expected ΔE00) from the published CIEDE2000 test dataset.
    let cases = [
        ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
        ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
        ([50.0, 2.8361, -74.0200], [50.0, 0.0, -82.7485], 3.4412),
    ];
    for (lab1, lab2, expected) in cases {
        let de = ciede2000(lab1, lab2);
        assert!(
            (de - expected).abs() < 1e-4,
            "ΔE00({lab1:?}, {lab2:?}) = {de}, expected {expected}"
        );
        let sym = ciede2000(lab2, lab1);
        assert!((de - sym).abs() < 1e-9, "ΔE00 must be symmetric");
    }
}

#[test]
fn srgb_to_lab_anchors() {
    let [l, a, b] = srgb_to_lab([255, 255, 255, 255]);
    assert!((l - 100.0).abs() < 0.01, "white L={l}");
    assert!(a.abs() < 0.01 && b.abs() < 0.01, "white a={a} b={b}");
    let [l, a, b] = srgb_to_lab([0, 0, 0, 255]);
    assert!(
        l.abs() < 1e-6 && a.abs() < 1e-6 && b.abs() < 1e-6,
        "black {l} {a} {b}"
    );
}

// ── Heatmap emission ──────────────────────────────────────────────────────────

#[test]
fn heatmap_marks_the_defect_hot() {
    let golden = solid(128, 128, [255, 255, 255, 255]);
    let mut candidate = golden.clone();
    for y in 8..24 {
        for x in 8..24 {
            candidate.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("heat.png");
    emit_heatmap(&golden, &candidate, &path).unwrap();
    let heat = image::open(&path).unwrap().to_rgba8();
    let hot = heat.get_pixel(16, 16).0;
    let cold = heat.get_pixel(100, 100).0;
    assert!(
        hot[0] > cold[0].saturating_add(50) || (hot[0] > 200 && hot[1] < 100),
        "defect pixel must be visibly hotter: hot={hot:?} cold={cold:?}"
    );
}
