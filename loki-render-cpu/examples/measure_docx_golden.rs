// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Measures Loki's CPU candidate render of a DOCX against a **Word** golden
//! directory, page by page, at the calibrated tolerance.
//!
//! The OOXML counterpart of `measure_odf_golden`. It exists separately because
//! the golden side is produced differently: LibreOffice can be driven headless
//! (`scripts/generate-odf-goldens.sh`), Word cannot, so its PDFs are printed on
//! a machine with Office and ingested by `scripts/generate-office-goldens.sh`.
//! The rasterizer and the differ are the same for both, so the two scores are
//! directly comparable.
//!
//! Unlike the gate (`tests/visual_golden_docx.rs`), which asserts on the first
//! failing page, this reports **every** page. A gate answers "may this land";
//! triage needs the distribution — one bad page among fourteen is a different
//! problem from fourteen mediocre ones, and the two are indistinguishable from
//! the gate's output.
//!
//! ```text
//! cargo run -p loki-render-cpu --example measure_docx_golden -- \
//!     appthere-conformance/fixtures/docx/iris-blueprint.docx \
//!     appthere-conformance/goldens/docx/iris-blueprint
//! # DUMP_DIR=/tmp/cand  writes candidate-N.png beside the scores
//! ```
//!
//! **Reading the result.** A low score is not automatically a Loki defect —
//! check the reported substitutions first. A fixture naming a face the
//! reference app has and this machine does not compares two different
//! typefaces, which is a font-availability problem wearing a fidelity
//! problem's clothes.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use appthere_conformance::CONFORMANCE_DPI;
use appthere_conformance::golden::{Tolerance, compare_pages};
use loki_doc_model::io::DocumentImport;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_render_cpu::render_page;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(fixture), Some(golden_dir)) = (args.next(), args.next()) else {
        eprintln!("usage: measure_docx_golden <fixture.docx> <golden-dir>");
        std::process::exit(2);
    };
    let fixture = PathBuf::from(fixture);
    let golden_dir = PathBuf::from(golden_dir);

    let Ok(bytes) = std::fs::read(&fixture) else {
        eprintln!("cannot read {}", fixture.display());
        std::process::exit(1);
    };
    let doc = match DocxImport::import(Cursor::new(bytes), DocxImportOptions::default()) {
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
    let mut failed = 0usize;
    let mut worst_overall = f64::MAX;
    while let Some(golden_png) = existing(&golden_dir, page) {
        let Ok(candidate) = render_page(&layout, page - 1, CONFORMANCE_DPI) else {
            println!("page {page:>3}: golden present, Loki has no such page");
            failed += 1;
            page += 1;
            continue;
        };
        let Ok(golden) = image::open(&golden_png).map(|g| g.to_rgba8()) else {
            println!("page {page:>3}: golden did not decode");
            failed += 1;
            page += 1;
            continue;
        };
        let w = golden.width().min(candidate.width());
        let h = golden.height().min(candidate.height());
        let g = image::imageops::crop_imm(&golden, 0, 0, w, h).to_image();
        let c = image::imageops::crop_imm(&candidate, 0, 0, w, h).to_image();
        match compare_pages(&g, &c, Tolerance::calibrated()) {
            Ok(report) => {
                let (s, d, r) = report.worst.map_or((f64::NAN, f64::NAN, (0, 0)), |x| {
                    (x.ssim, x.delta_e, x.region)
                });
                println!(
                    "page {page:>3}: {} ssim={s:.4} dE={d:.3} worst-region={r:?}",
                    if report.passed { "PASS" } else { "FAIL" }
                );
                if !report.passed {
                    failed += 1;
                }
                if s < worst_overall {
                    worst_overall = s;
                }
            }
            Err(err) => {
                println!("page {page:>3}: compare failed: {err:?}");
                failed += 1;
            }
        }
        if let Ok(dir) = std::env::var("DUMP_DIR") {
            let _ = std::fs::create_dir_all(&dir);
            let _ = candidate.save(format!("{dir}/candidate-{page:02}.png"));
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
    println!("{compared} page(s) compared, {failed} failing; worst ssim={worst_overall:.4}");
}

/// The golden PNG for `page`, zero-padded or not.
///
/// `generate-office-goldens.sh` pads the page number to the width of the page
/// count, so a 14-page golden is `page-01.png` and a 7-page one `page-1.png`.
/// Trying both is what lets one tool read either.
fn existing(dir: &Path, page: usize) -> Option<PathBuf> {
    for name in [format!("page-{page}.png"), format!("page-{page:02}.png")] {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}
