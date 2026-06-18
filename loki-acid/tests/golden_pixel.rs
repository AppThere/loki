// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Golden pixel diff (SSIM) — active once `goldens/` and `renders/` are
//! populated.
//!
//! For every `goldens/<stem>/page-NNN.png` that has a matching
//! `renders/<stem>/page-NNN.png`, the page is diffed with mean SSIM and asserted
//! to meet [`SSIM_THRESHOLD`]. When no goldens (or no matching renders) exist
//! the test is a documented no-op so the suite stays green until references are
//! supplied — see `README.md` for the workflow that produces both trees.

use loki_acid::diff::mean_ssim;
use loki_acid::fixtures::Fixture;
use loki_acid::golden::{golden_pages, load_png, render_for};

/// Minimum acceptable mean SSIM between a Loki render and its golden reference.
const SSIM_THRESHOLD: f64 = 0.98;

#[test]
fn golden_pages_match_within_ssim_threshold() {
    let mut compared = 0usize;
    let mut failures = Vec::new();

    for &fixture in Fixture::all() {
        for golden_path in golden_pages(fixture) {
            let Some(render_path) = render_for(&golden_path) else {
                continue;
            };
            if !render_path.exists() {
                eprintln!(
                    "skip {}: no Loki render at {}",
                    golden_path.display(),
                    render_path.display()
                );
                continue;
            }

            let golden = load_png(&golden_path).expect("load golden");
            let render = load_png(&render_path).expect("load render");
            match mean_ssim(&golden, &render) {
                Ok(ssim) => {
                    compared += 1;
                    if ssim < SSIM_THRESHOLD {
                        failures.push(format!(
                            "{} vs {}: SSIM {ssim:.4} < {SSIM_THRESHOLD}",
                            golden_path.display(),
                            render_path.display()
                        ));
                    }
                }
                Err(e) => failures.push(format!("{}: {e}", golden_path.display())),
            }
        }
    }

    if compared == 0 {
        eprintln!(
            "golden pixel diff: no golden/render pairs found — populate \
             goldens/<stem>/page-NNN.png and renders/<stem>/page-NNN.png \
             (see README.md). Skipping."
        );
        return;
    }

    assert!(
        failures.is_empty(),
        "{} of {compared} page(s) below SSIM threshold:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("golden pixel diff: {compared} page(s) within SSIM threshold");
}
