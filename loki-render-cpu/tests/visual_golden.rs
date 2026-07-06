// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The visual-goldens axis over the committed ODF golden set, at the
//! **calibrated** tolerance (Spec 02 M5 acceptance: a correct fixture
//! passes, a mis-rendered one fails, and the threshold traces to the
//! calibration record, not a literal).
//!
//! All three committed fixtures pass at the calibrated tolerance. The
//! original `para-carlito` divergence was root-caused (2026-07-05) to loki
//! kerning unconditionally while the reference apps default kerning OFF;
//! `StyleSpan::kerning` now honours the document property with a
//! reference-matching default (see `loki-layout/tests/kerning_applied.rs`
//! and goldens/CALIBRATION.md).

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

/// Formerly the kerning-gap canary: this fixture diverged while loki kerned
/// text the reference apps leave unkerned (gap #23, resolved 2026-07-05).
/// Now a full member of the passing golden set.
#[test]
fn para_carlito_matches_its_golden() {
    let (passed, worst) = compare("para-carlito");
    assert!(
        passed,
        "para-carlito must pass calibrated tolerance; {worst}"
    );
}
