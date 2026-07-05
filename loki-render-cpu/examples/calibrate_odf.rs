// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Spec 02 §7.4 / D5 calibration pass: measures the natural
//! cross-renderer noise floor between the committed LibreOffice goldens and
//! Loki's `vello_cpu` candidate renders, over the baseline fixture set
//! believed correct in both engines.
//!
//! Prints per-fixture and aggregate region-score distributions (SSIM and
//! CIEDE2000 ΔE) — the data the committed calibration record
//! (`appthere-conformance/goldens/CALIBRATION.md`) and the calibrated
//! `Tolerance` derive from. Also emits heatmaps for visual inspection.
//!
//! Run: `cargo run -p loki-render-cpu --example calibrate_odf`

use std::io::Cursor;
use std::path::Path;

use appthere_conformance::CONFORMANCE_DPI;
use appthere_conformance::golden::{Tolerance, compare_pages, emit_heatmap};
use loki_doc_model::io::DocumentImport;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_render_cpu::render_page;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let fixtures = root.join("appthere-conformance/fixtures/odt");
    let goldens = root.join("appthere-conformance/goldens/odt");
    let scratch = std::env::temp_dir().join("loki-calibration");
    std::fs::create_dir_all(&scratch).expect("scratch dir");

    let mut all_ssim: Vec<f64> = Vec::new();
    let mut all_de: Vec<f64> = Vec::new();

    for entry in std::fs::read_dir(&fixtures).expect("fixtures dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "odt") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let golden_png = goldens.join(&stem).join("page-1.png");
        if !golden_png.exists() {
            eprintln!("skipping {stem}: no golden committed");
            continue;
        }

        // Candidate: import → layout (bundled faces registered) → CPU render.
        let bytes = std::fs::read(&path).expect("read fixture");
        let doc = OdtImport::import(Cursor::new(bytes), OdtImportOptions::default())
            .expect("fixture must import");
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
        let golden = image::open(&golden_png).expect("golden decodes").to_rgba8();

        // The two sides may differ by a rounding pixel; crop to the common
        // area (recorded in the calibration doc).
        let w = golden.width().min(candidate.width());
        let h = golden.height().min(candidate.height());
        let golden = image::imageops::crop_imm(&golden, 0, 0, w, h).to_image();
        let candidate_c = image::imageops::crop_imm(&candidate, 0, 0, w, h).to_image();

        // Permissive tolerance: this pass MEASURES; it does not judge.
        let report = compare_pages(
            &golden,
            &candidate_c,
            Tolerance {
                min_ssim: 0.0,
                max_delta_e: f64::MAX,
            },
        )
        .expect("compare");

        let mut ssim: Vec<f64> = report.regions.iter().map(|r| r.ssim).collect();
        let mut de: Vec<f64> = report.regions.iter().map(|r| r.delta_e).collect();
        ssim.sort_by(f64::total_cmp);
        de.sort_by(f64::total_cmp);
        println!(
            "{stem}: {} regions ({}x{} px)\n  ssim  min={:.4} p1={:.4} p5={:.4} median={:.4}\n  ΔE    max={:.3} p99={:.3} p95={:.3} median={:.3}",
            ssim.len(),
            w,
            h,
            ssim.first().unwrap(),
            percentile(&ssim, 0.01),
            percentile(&ssim, 0.05),
            percentile(&ssim, 0.50),
            de.last().unwrap(),
            percentile(&de, 0.99),
            percentile(&de, 0.95),
            percentile(&de, 0.50),
        );
        let heatmap = scratch.join(format!("{stem}-heatmap.png"));
        emit_heatmap(&golden, &candidate_c, &heatmap).expect("heatmap");
        println!("  heatmap: {}", heatmap.display());
        // Optional side-by-side dump for offline inspection of a divergence.
        if let Ok(dir) = std::env::var("CALIBRATE_DUMP_DIR") {
            let dir = Path::new(&dir);
            std::fs::create_dir_all(dir).expect("dump dir");
            golden.save(dir.join(format!("{stem}-golden.png"))).ok();
            candidate_c
                .save(dir.join(format!("{stem}-candidate.png")))
                .ok();
        }

        all_ssim.extend(ssim);
        all_de.extend(de);
    }

    all_ssim.sort_by(f64::total_cmp);
    all_de.sort_by(f64::total_cmp);
    println!(
        "\nAGGREGATE over {} regions:\n  ssim  min={:.4} p1={:.4} p5={:.4}\n  ΔE    max={:.3} p99={:.3} p95={:.3}",
        all_ssim.len(),
        all_ssim.first().unwrap(),
        percentile(&all_ssim, 0.01),
        percentile(&all_ssim, 0.05),
        all_de.last().unwrap(),
        percentile(&all_de, 0.99),
        percentile(&all_de, 0.95),
    );
}
