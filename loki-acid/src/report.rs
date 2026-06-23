// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-fixture and aggregate fidelity reports (the rasteriser-free layer).

use serde::{Deserialize, Serialize};

use crate::catalog::{self, Format};
use crate::fixtures::{Fixture, Imported};
use crate::pages::{GlyphCoverage, glyph_coverage, paginate};

/// Structural analysis of a single fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureReport {
    /// Fixture asset file name.
    pub fixture: String,
    /// Format under test.
    pub format: Format,
    /// Whether an importer exists for this format.
    pub has_importer: bool,
    /// Whether import succeeded.
    pub import_ok: bool,
    /// Import error, if any.
    pub import_error: Option<String>,
    /// Paginated page count (word-processing documents only).
    pub page_count: Option<usize>,
    /// Worksheet count (spreadsheets only).
    pub sheet_count: Option<usize>,
    /// Slide count (presentations only).
    pub slide_count: Option<usize>,
    /// Glyph coverage (word-processing documents only).
    pub glyph_coverage: Option<GlyphCoverage>,
    /// Number of catalogued acid cases targeting this format.
    pub catalogued_cases: usize,
}

/// Analyses one fixture: import, then paginate + glyph-scan (documents) or count
/// sheets (workbooks).
#[must_use]
pub fn analyze_fixture(fixture: Fixture) -> FixtureReport {
    let format = fixture.format();
    let mut report = FixtureReport {
        fixture: fixture.asset_name().to_string(),
        format,
        has_importer: fixture.has_importer(),
        import_ok: false,
        import_error: None,
        page_count: None,
        sheet_count: None,
        slide_count: None,
        glyph_coverage: None,
        catalogued_cases: catalog::cases_for(format).len(),
    };

    match fixture.import() {
        Ok(Imported::Document(doc)) => {
            report.import_ok = true;
            let layout = paginate(&doc);
            report.page_count = Some(layout.pages.len());
            report.glyph_coverage = Some(glyph_coverage(&layout));
        }
        Ok(Imported::Workbook(wb)) => {
            report.import_ok = true;
            report.sheet_count = Some(wb.sheets.len());
        }
        Ok(Imported::Presentation(p)) => {
            report.import_ok = true;
            report.slide_count = Some(p.slide_count());
        }
        Err(e) => report.import_error = Some(e),
    }
    report
}

/// The aggregate report across every fixture, plus catalog totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcidReport {
    /// Per-fixture structural reports.
    pub fixtures: Vec<FixtureReport>,
    /// Total catalogued acid cases.
    pub total_cases: usize,
}

/// Runs [`analyze_fixture`] over every supplied fixture.
#[must_use]
pub fn run() -> AcidReport {
    AcidReport {
        fixtures: Fixture::all().iter().map(|&f| analyze_fixture(f)).collect(),
        total_cases: catalog::all_cases().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serialises_to_json() {
        let report = AcidReport {
            fixtures: vec![],
            total_cases: 0,
        };
        let json = serde_json::to_string(&report).expect("serialise");
        assert!(json.contains("total_cases"));
    }
}
