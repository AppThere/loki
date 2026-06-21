// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Rasteriser-free structural canaries: import, pagination, and glyph coverage.
//!
//! These run in any environment. Strict tofu / page-count-vs-reference checks
//! that depend on the host font set or on golden references live in
//! `golden_pixel.rs` (and the `#[ignore]`d strict test below).

use loki_acid::fixtures::{Fixture, Imported};
use loki_acid::pages::{glyph_coverage, paginate};
use loki_acid::report;

#[test]
fn supported_fixtures_import_cleanly() {
    for &fixture in Fixture::all() {
        if !fixture.has_importer() {
            continue;
        }
        let outcome = fixture.import();
        assert!(
            outcome.is_ok(),
            "{} failed to import: {:?}",
            fixture.asset_name(),
            outcome.err()
        );
    }
}

#[test]
fn unsupported_fixtures_are_documented() {
    for fixture in [Fixture::Odp, Fixture::Odg] {
        assert!(!fixture.has_importer());
        assert!(
            fixture.import().is_err(),
            "{} unexpectedly imported",
            fixture.asset_name()
        );
    }
}

#[test]
fn word_processing_fixtures_paginate() {
    for fixture in [Fixture::Docx, Fixture::Odt] {
        let Ok(Imported::Document(doc)) = fixture.import() else {
            panic!("{} did not import as a document", fixture.asset_name());
        };
        let layout = paginate(&doc);
        assert!(
            !layout.pages.is_empty(),
            "{} produced zero pages",
            fixture.asset_name()
        );
        // Coverage is computable and self-consistent.
        let cov = glyph_coverage(&layout);
        assert!(cov.notdef_glyphs <= cov.total_glyphs);
    }
}

#[test]
fn spreadsheet_fixtures_have_sheets() {
    for fixture in [Fixture::Xlsx, Fixture::Ods] {
        let Ok(Imported::Workbook(wb)) = fixture.import() else {
            panic!("{} did not import as a workbook", fixture.asset_name());
        };
        assert!(
            !wb.sheets.is_empty(),
            "{} has no sheets",
            fixture.asset_name()
        );
    }
}

#[test]
fn presentation_fixture_has_slides() {
    let Ok(Imported::Presentation(p)) = Fixture::Pptx.import() else {
        panic!("acid_pptx.pptx did not import as a presentation");
    };
    assert!(
        p.slide_count() >= 1,
        "acid_pptx.pptx imported with no slides"
    );
    // The aggregate report must surface the slide count for the PPTX fixture.
    let report = report::run();
    let pptx = report
        .fixtures
        .iter()
        .find(|f| f.format == loki_acid::Format::Pptx)
        .expect("pptx fixture in report");
    assert_eq!(pptx.slide_count, Some(p.slide_count()));
    assert!(pptx.import_ok, "pptx fixture failed structural import");
}

#[test]
fn aggregate_report_covers_all_fixtures() {
    let report = report::run();
    assert_eq!(report.fixtures.len(), Fixture::all().len());
    assert!(report.total_cases >= 130, "catalog unexpectedly small");
    // Every supplied document fixture reports a page count.
    for f in &report.fixtures {
        if matches!(f.format, loki_acid::Format::Docx | loki_acid::Format::Odt) {
            assert!(f.page_count.is_some(), "{} missing page count", f.fixture);
        }
    }
}

/// Strict glyph-coverage gate. Requires the host to have the fixtures' declared
/// fonts (or metric-compatible substitutes) installed, so it is opt-in:
///
/// ```text
/// cargo test -p loki-acid -- --ignored no_tofu
/// ```
#[test]
#[ignore = "host-font-dependent; run on a fully-fonted reference machine"]
fn no_tofu_glyphs() {
    for fixture in [Fixture::Docx, Fixture::Odt] {
        let Ok(Imported::Document(doc)) = fixture.import() else {
            continue;
        };
        let cov = glyph_coverage(&paginate(&doc));
        assert!(
            !cov.has_tofu(),
            "{}: {} tofu glyph(s) on pages {:?}",
            fixture.asset_name(),
            cov.notdef_glyphs,
            cov.pages_with_tofu
        );
    }
}
