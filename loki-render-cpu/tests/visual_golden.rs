// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The visual-goldens axis over the committed ODF golden set, at the
//! **calibrated** tolerance (Spec 02 M5 acceptance: a correct fixture
//! passes, a mis-rendered one fails, and the threshold traces to the
//! calibration record, not a literal).
//!
//! Advisory status: this suite runs in CI as ordinary tests, but the gate is
//! kept honest by pinning the *known* divergence (`para-carlito`, fidelity
//! gap #23 — kerning) as an expected failure rather than hiding it. When
//! kerning lands in `loki-layout`, that canary flips and must be promoted to
//! a passing assertion (and the calibration re-run).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use appthere_conformance::CONFORMANCE_DPI;
use appthere_conformance::golden::{Tolerance, compare_pages};
use loki_doc_model::io::DocumentImport;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_render_cpu::render_page;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Renders the fixture's first page and compares against its committed
/// golden at the calibrated tolerance. Returns the pass verdict and the
/// worst region for diagnostics.
fn compare(stem: &str) -> (bool, String) {
    let fixture = root().join(format!("appthere-conformance/fixtures/odt/{stem}.odt"));
    let golden_png = root().join(format!(
        "appthere-conformance/goldens/odt/{stem}/page-1.png"
    ));

    let bytes = std::fs::read(&fixture).expect("fixture exists (committed)");
    let doc = OdtImport::import(Cursor::new(bytes), OdtImportOptions::default())
        .expect("fixture imports");
    let mut resources = FontResources::new();
    for blob in loki_fonts::fallback_font_blobs() {
        resources.register_font(blob.to_vec());
    }
    let layout = match layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) {
        DocumentLayout::Paginated(p) => p,
        other => panic!("expected paginated layout, got {other:?}"),
    };
    let candidate = render_page(&layout, 0, CONFORMANCE_DPI).expect("candidate render");
    let golden = image::open(&golden_png)
        .expect("golden decodes (committed)")
        .to_rgba8();

    // ≤1 px DPI-rounding difference between the two pipelines (recorded in
    // CALIBRATION.md); crop to the common area.
    let w = golden.width().min(candidate.width());
    let h = golden.height().min(candidate.height());
    let golden = image::imageops::crop_imm(&golden, 0, 0, w, h).to_image();
    let candidate = image::imageops::crop_imm(&candidate, 0, 0, w, h).to_image();

    let report = compare_pages(&golden, &candidate, Tolerance::calibrated()).expect("compare");
    let worst = report
        .worst
        .map(|r| {
            format!(
                "worst region {:?}: ssim={:.4} delta_e={:.3}",
                r.region, r.ssim, r.delta_e
            )
        })
        .unwrap_or_default();
    (report.passed, worst)
}

#[test]
fn styles_tinos_matches_its_golden() {
    let (passed, worst) = compare("styles-tinos");
    assert!(
        passed,
        "styles-tinos must pass calibrated tolerance; {worst}"
    );
}

#[test]
fn para_gelasio_matches_its_golden() {
    let (passed, worst) = compare("para-gelasio");
    assert!(
        passed,
        "para-gelasio must pass calibrated tolerance; {worst}"
    );
}

/// Expected-divergence canary: LibreOffice applies Carlito's kern pairs;
/// Loki's layout does not yet (fidelity gap #23), so lines drift and wrap
/// differently. When kerning lands this assertion FAILS — flip it to a
/// passing golden check and re-run the calibration (CALIBRATION.md).
#[test]
fn para_carlito_divergence_is_the_known_kerning_gap() {
    let (passed, worst) = compare("para-carlito");
    assert!(
        !passed,
        "para-carlito unexpectedly passes — kerning may have landed; \
         promote this canary to a passing check and re-calibrate"
    );
    eprintln!("known kerning-gap divergence (fidelity #23): {worst}");
}
