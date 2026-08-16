// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Measures Loki's CPU candidate render of an arbitrary ODT against a
//! LibreOffice golden directory, page by page, at the calibrated tolerance.
//!
//! This is the "can this fixture join the pixel gate?" instrument. The
//! committed conformance fixtures already pass (see
//! `tests/visual_golden.rs`); this answers the same question for a *candidate*
//! fixture before anyone commits a golden for it.
//!
//! Producing the golden side (see `scripts/generate-odf-goldens.sh` for the
//! sanctioned pipeline and its version pinning):
//!
//! ```text
//! soffice --headless --convert-to pdf --outdir "$W" <fixture>.odt
//! cargo run -p appthere-conformance --example rasterize_pdf -- "$W/<stem>.pdf" "$W/png" page
//! cargo run -p loki-render-cpu --example measure_odf_golden -- <fixture>.odt "$W/png"
//! ```
//!
//! **Reading the result.** A low score is not automatically a Loki defect. The
//! first thing to check is whether the fixture pins its fonts: a document with
//! no `<style:default-style>` inherits each *application's* own default face,
//! so LibreOffice and Loki will disagree everywhere for a reason that has
//! nothing to do with either engine's fidelity. The committed conformance
//! fixtures are font-pinned by name (`para-carlito`, `para-gelasio`,
//! `styles-tinos`) for exactly this reason.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use appthere_conformance::CONFORMANCE_DPI;
use appthere_conformance::golden::{Tolerance, compare_pages};
use loki_doc_model::io::DocumentImport;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_render_cpu::render_page;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(fixture), Some(golden_dir)) = (args.next(), args.next()) else {
        eprintln!("usage: measure_odf_golden <fixture.odt> <golden-dir>");
        eprintln!("  <golden-dir> holds page-1.png, page-2.png, … at CONFORMANCE_DPI");
        std::process::exit(2);
    };
    let fixture = PathBuf::from(fixture);
    let golden_dir = PathBuf::from(golden_dir);

    let Ok(bytes) = std::fs::read(&fixture) else {
        eprintln!("cannot read {}", fixture.display());
        std::process::exit(1);
    };
    let doc = match OdtImport::import(Cursor::new(bytes), OdtImportOptions::default()) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("import failed: {err:?}");
            std::process::exit(1);
        }
    };

    let mut resources = FontResources::new();
    for blob in loki_fonts::fallback_font_blobs() {
        resources.register_font(blob.to_vec());
    }
    // Loki records what it could not resolve. Print it: attributing a low score
    // to the engine while a direct report of a font substitution was available
    // and unread is the easy mistake here.
    resources.begin_substitution_run();

    let DocumentLayout::Paginated(layout) = layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) else {
        eprintln!("expected a paginated layout");
        std::process::exit(1);
    };

    let subs = resources.take_substitution_run();
    println!("fixture:       {}", fixture.display());
    println!("loki pages:    {}", layout.pages.len());
    println!("substitutions: {subs:?}");

    let mut page = 1usize;
    let mut compared = 0usize;
    let mut passed_all = true;
    while let Some(golden_png) = existing(&golden_dir, page) {
        let Ok(candidate) = render_page(&layout, page - 1, CONFORMANCE_DPI) else {
            println!("page {page}: golden present, Loki has no such page");
            passed_all = false;
            page += 1;
            continue;
        };
        let Ok(golden) = image::open(&golden_png).map(|g| g.to_rgba8()) else {
            println!("page {page}: golden did not decode");
            passed_all = false;
            page += 1;
            continue;
        };
        // ≤1 px DPI-rounding difference between the two pipelines (recorded in
        // goldens/CALIBRATION.md); crop to the common area, as the tests do.
        let w = golden.width().min(candidate.width());
        let h = golden.height().min(candidate.height());
        let g = image::imageops::crop_imm(&golden, 0, 0, w, h).to_image();
        let c = image::imageops::crop_imm(&candidate, 0, 0, w, h).to_image();
        match compare_pages(&g, &c, Tolerance::calibrated()) {
            Ok(report) => {
                let worst = report
                    .worst
                    .map(|r| {
                        format!(
                            "ssim={:.4} delta_e={:.3} at {:?}",
                            r.ssim, r.delta_e, r.region
                        )
                    })
                    .unwrap_or_else(|| "no regions".into());
                println!("page {page}: passed={} worst: {worst}", report.passed);
                passed_all &= report.passed;
            }
            Err(err) => {
                println!("page {page}: compare failed: {err:?}");
                passed_all = false;
            }
        }
        compared += 1;
        page += 1;
    }

    if compared == 0 {
        println!(
            "no goldens found in {} — nothing compared",
            golden_dir.display()
        );
        std::process::exit(1);
    }
    println!("{compared} page(s) compared; gateable = {passed_all}");
}

/// The golden PNG for `page`, if it exists.
fn existing(dir: &Path, page: usize) -> Option<PathBuf> {
    let p = dir.join(format!("page-{page}.png"));
    p.exists().then_some(p)
}
