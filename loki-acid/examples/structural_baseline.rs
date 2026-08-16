// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Prints the ACID structural baseline (page/sheet/slide counts + glyph
//! coverage) as a stable, diffable table.
//!
//! This exists for the Dioxus 0.8 migration (docs/dioxus-0.8-migration.md,
//! Phase 0.4): the structural canaries assert that the numbers are *acceptable*,
//! which is a ceiling. A migration also needs a **floor** — the numbers as they
//! actually stand — or a regression that lands inside the assertion's tolerance
//! is invisible. Run before and after a stack change and diff the output.

fn main() {
    let report = loki_acid::report::run();
    println!(
        "{:<34} {:>5} {:>6} {:>6} {:>7} {:>9}",
        "fixture", "pages", "sheets", "slides", "glyphs", "coverage"
    );
    for f in &report.fixtures {
        let n = |v: Option<usize>| v.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let (glyphs, cov) = match &f.glyph_coverage {
            Some(g) => (
                g.total_glyphs.to_string(),
                format!("{:.4}", g.coverage_ratio()),
            ),
            None => ("-".into(), "-".into()),
        };
        println!(
            "{:<34} {:>5} {:>6} {:>6} {:>7} {:>9}",
            f.fixture,
            n(f.page_count),
            n(f.sheet_count),
            n(f.slide_count),
            glyphs,
            cov
        );
    }
}
