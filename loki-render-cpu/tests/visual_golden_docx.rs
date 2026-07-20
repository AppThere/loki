// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The DOCX visual-golden axis: compare Loki's render of each committed OOXML
//! corpus fixture against Microsoft Word's render at the calibrated tolerance.
//!
//! Word cannot be automated headlessly, so its goldens are captured manually
//! (open the fixture in Word, print to PDF, run
//! `scripts/generate-office-goldens.sh`) into
//! `appthere-conformance/goldens/docx/<stem>/page-N.png`. Until those PNGs are
//! committed each fixture's golden dir holds only a `PENDING.txt`, so
//! [`golden_pages`] returns empty and the comparison is a **documented no-op**
//! that keeps the suite green — mirroring how the ODF golden axis began. The
//! moment real goldens land, these tests enforce the calibrated SSIM/ΔE
//! thresholds page-by-page with no further wiring.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use appthere_conformance::CONFORMANCE_DPI;
use appthere_conformance::golden::{Tolerance, compare_pages, golden_pages, load_png};
use image::RgbaImage;
use loki_doc_model::io::DocumentImport;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_render_cpu::render_document;

fn conformance_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../appthere-conformance")
}

/// Import the DOCX fixture and render every page through the pinned CPU
/// candidate path at [`CONFORMANCE_DPI`] — the exact geometry the goldens use.
fn render_candidate(stem: &str) -> Vec<RgbaImage> {
    let fixture = conformance_root().join(format!("fixtures/docx/{stem}.docx"));
    let bytes = std::fs::read(&fixture).expect("fixture exists (committed)");
    let doc = DocxImport::import(Cursor::new(bytes), DocxImportOptions::default())
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
    render_document(&layout, CONFORMANCE_DPI).expect("candidate render")
}

/// Compare a fixture against its committed goldens at the calibrated tolerance.
/// No-op (green) while the golden tree is empty.
fn compare_fixture(stem: &str) {
    let golden_root = conformance_root().join("goldens/docx");
    let goldens = golden_pages(&golden_root, stem);
    if goldens.is_empty() {
        eprintln!(
            "visual_golden_docx: no goldens committed for {stem} yet \
             (see goldens/docx/{stem}/PENDING.txt) — skipping"
        );
        return;
    }

    let candidate = render_candidate(stem);
    assert!(
        candidate.len() >= goldens.len(),
        "{stem}: page-count drift — {} golden page(s) but Loki produced {}",
        goldens.len(),
        candidate.len()
    );

    for (i, golden_path) in goldens.iter().enumerate() {
        let golden = load_png(golden_path).expect("golden decodes");
        let cand = &candidate[i];
        // ≤1 px DPI-rounding difference between the two pipelines; crop to the
        // common area (as the ODF axis does).
        let w = golden.width().min(cand.width());
        let h = golden.height().min(cand.height());
        let golden_c = image::imageops::crop_imm(&golden, 0, 0, w, h).to_image();
        let cand_c = image::imageops::crop_imm(cand, 0, 0, w, h).to_image();
        let report = compare_pages(&golden_c, &cand_c, Tolerance::calibrated()).expect("compare");
        let worst = report
            .worst
            .map(|r| {
                format!(
                    "worst region {:?}: ssim={:.4} ΔE={:.3}",
                    r.region, r.ssim, r.delta_e
                )
            })
            .unwrap_or_default();
        assert!(
            report.passed,
            "{stem} page {} must pass calibrated tolerance; {worst}",
            i + 1
        );
    }
}

#[test]
fn acid_docx_matches_its_golden() {
    compare_fixture("acid-docx");
}

#[test]
fn iris_blueprint_matches_its_golden() {
    compare_fixture("iris-blueprint");
}

#[test]
fn acid2_docx_matches_its_golden() {
    compare_fixture("acid2-docx");
}
