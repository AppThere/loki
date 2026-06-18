// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Emits the ACID structural fidelity report as JSON.
//!
//! ```text
//! cargo run -p loki-acid --example acid_report          # pretty JSON to stdout
//! cargo run -p loki-acid --example acid_report -- out.json
//! ```
//!
//! The report imports every supplied fixture, paginates the word-processing
//! documents, and records page counts and glyph coverage (tofu) — the
//! rasteriser-free canaries from the test plan.

fn main() {
    let report = loki_acid::report::run();
    let json = serde_json::to_string_pretty(&report).expect("serialise report");

    // Human-readable summary to stderr so stdout stays clean JSON.
    eprintln!(
        "ACID fidelity report — {} catalogued cases",
        report.total_cases
    );
    for f in &report.fixtures {
        let status = if f.import_ok {
            "ok"
        } else if f.has_importer {
            "IMPORT FAILED"
        } else {
            "no importer"
        };
        let detail = match (f.page_count, f.sheet_count) {
            (Some(p), _) => format!("{p} page(s)"),
            (_, Some(s)) => format!("{s} sheet(s)"),
            _ => f.import_error.clone().unwrap_or_default(),
        };
        let tofu = f
            .glyph_coverage
            .as_ref()
            .map(|c| {
                if c.has_tofu() {
                    format!(" — TOFU on pages {:?}", c.pages_with_tofu)
                } else if c.total_glyphs > 0 {
                    format!(" — {} glyphs, full coverage", c.total_glyphs)
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        eprintln!("  {:14} [{:>12}] {detail}{tofu}", f.fixture, status);
    }

    match std::env::args().nth(1) {
        Some(path) => {
            std::fs::write(&path, json).expect("write report");
            eprintln!("wrote {path}");
        }
        None => println!("{json}"),
    }
}
