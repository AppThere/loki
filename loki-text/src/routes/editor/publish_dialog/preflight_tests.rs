// SPDX-License-Identifier: Apache-2.0

//! Tests for the EPUB preflight.

use super::super::audit::{ImageAudit, TableAudit, heading_structure, image_audit, table_audit};
use super::*;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::inline::LinkTarget;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::LanguageTag;

fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    d.sections = vec![Section {
        blocks,
        ..Default::default()
    }];
    d
}

/// A document with everything the package needs.
fn publishable() -> Document {
    let mut d = doc(vec![Block::Heading(
        1,
        NodeAttr::default(),
        vec![Inline::Str("One".to_string())],
    )]);
    d.meta.title = Some("The long afternoon".to_string());
    d.meta.creator = Some("M. Halloran".to_string());
    d.meta.language = Some(LanguageTag::new("en-GB"));
    d.meta.dublin_core.identifier = Some("urn:uuid:8f14e45f".to_string());
    d.meta.dublin_core.publisher = Some("Harbour Press".to_string());
    d
}

/// Note 30: warnings never block. A document missing only its publisher still
/// publishes — that is the whole distinction the severity carries.
#[test]
fn warnings_do_not_block_the_publish() {
    let mut d = publishable();
    d.meta.dublin_core.publisher = None;
    let report = run(&d);

    assert!(report.warnings() > 0, "the missing publisher is reported");
    assert_eq!(report.errors(), 0);
    assert!(report.can_publish(), "a warning must not block");
}

/// The package-level requirements are errors, and an error *does* block —
/// otherwise the severity distinction means nothing.
#[test]
fn a_missing_package_requirement_blocks() {
    for strip in [0, 1] {
        let mut d = publishable();
        if strip == 0 {
            d.meta.title = None;
        } else {
            d.meta.language = None;
        }
        let report = run(&d);
        assert!(report.errors() > 0, "case {strip} should be an error");
        assert!(!report.can_publish(), "case {strip} should block");
    }
}

/// Whitespace is not a title. A check that only tested `is_some` would pass a
/// package whose `dc:title` is a space.
#[test]
fn whitespace_does_not_satisfy_a_required_field() {
    let mut d = publishable();
    d.meta.title = Some("   ".to_string());
    assert!(!run(&d).can_publish());
}

/// A fully-specified document reports only passes.
#[test]
fn a_complete_document_passes_everything() {
    let report = run(&publishable());
    assert_eq!(report.errors(), 0);
    assert_eq!(report.warnings(), 0);
    assert!(report.passed() > 0);
    assert!(report.can_publish());
}

/// The rail's top row must be the one worth reading.
#[test]
fn findings_are_sorted_most_serious_first() {
    let mut d = publishable();
    d.meta.title = None;
    d.meta.dublin_core.publisher = None;
    let report = run(&d);

    let severities: Vec<Severity> = report.findings.iter().map(|f| f.severity).collect();
    let mut sorted = severities.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(severities, sorted);
    assert_eq!(severities.first(), Some(&Severity::Error));
}

/// Every failing finding names where to fix it, or the "open metadata" link has
/// nothing to point at.
#[test]
fn every_failure_names_where_to_fix_it() {
    let mut d = publishable();
    d.meta.title = None;
    d.meta.creator = None;
    for finding in run(&d).findings {
        if finding.severity != Severity::Pass {
            assert_ne!(finding.fix_in, FixIn::None, "{}", finding.message);
        }
    }
}

/// A skipped level leaves a hole in a reader's navigation; coming back up any
/// distance is fine, so only descents are flagged.
#[test]
fn heading_nesting_flags_skipped_descents_but_not_ascents() {
    let h = |level: u8| Block::Heading(level, NodeAttr::default(), vec![Inline::Str("x".into())]);

    let good = heading_structure(&doc(vec![h(1), h(2), h(3), h(1)]));
    assert_eq!(good.headings, 4);
    assert!(good.well_nested, "1→2→3→1 is well nested");

    let skipped = heading_structure(&doc(vec![h(1), h(3)]));
    assert!(!skipped.well_nested, "1→3 skips level 2");
}

/// A document with no headings has nothing to skip — vacuously well nested, and
/// reported separately as "no headings".
#[test]
fn a_document_with_no_headings_is_vacuously_well_nested() {
    let structure = heading_structure(&doc(Vec::new()));
    assert_eq!(structure.headings, 0);
    assert!(structure.well_nested);
}

/// A table needs both a caption and a header row to be usable by a reader; one
/// without the other is not enough.
#[test]
fn a_table_needs_both_a_caption_and_a_header_row() {
    let bare = Table::grid(2, 2);
    assert_eq!(
        table_audit(&doc(vec![Block::Table(Box::new(bare))])),
        TableAudit {
            total: 1,
            accessible: 0
        }
    );

    let mut captioned = Table::grid(2, 2);
    captioned.caption.full = vec![Inline::Str("Tide times".to_string())];
    assert_eq!(
        table_audit(&doc(vec![Block::Table(Box::new(captioned.clone()))])),
        TableAudit {
            total: 1,
            accessible: 0
        },
        "a caption alone is not enough"
    );

    let mut full = captioned;
    if let Some(body) = full.bodies.first_mut()
        && !body.body_rows.is_empty()
    {
        let head = body.body_rows.remove(0);
        full.head.rows.push(head);
    }
    assert_eq!(
        table_audit(&doc(vec![Block::Table(Box::new(full))])),
        TableAudit {
            total: 1,
            accessible: 1
        }
    );
}

/// An image with empty alt text tells a reader nothing, so it is not described.
#[test]
fn images_are_described_only_when_they_carry_alt_text() {
    let bare = Inline::Image(
        NodeAttr::default(),
        Vec::new(),
        LinkTarget::new("cover.png"),
    );
    let described = Inline::Image(
        NodeAttr::default(),
        vec![Inline::Str("A harbour at dusk".to_string())],
        LinkTarget::new("cover.png"),
    );
    assert_eq!(
        image_audit(&doc(vec![Block::Para(vec![bare, described])])),
        ImageAudit {
            total: 2,
            described: 1
        }
    );
}

/// A bare table only raises a warning once one exists — a document with no
/// tables should not be told its tables are fine.
#[test]
fn table_and_image_checks_are_omitted_when_there_are_none() {
    let report = run(&publishable());
    for finding in &report.findings {
        assert!(
            !finding.message.to_lowercase().contains("table"),
            "no table check on a table-less document: {}",
            finding.message
        );
    }
}

/// The Insert table dialog promotes the first row into `table.head.rows`, so a
/// walk that visited only `bodies` could not see the one structure this feature
/// set creates by default: an undescribed image in a header cell made the
/// preflight report that every image had alternative text.
#[test]
fn an_image_in_a_table_header_is_audited() {
    let mut table = Table::grid(1, 1);
    if let Some(body) = table.bodies.first_mut()
        && let Some(row) = body.body_rows.first_mut()
        && let Some(cell) = row.cells.first_mut()
    {
        cell.blocks = vec![Block::Para(vec![Inline::Image(
            NodeAttr::default(),
            Vec::new(),
            LinkTarget::new("cover.png"),
        )])];
    }
    // Promote it into the head, exactly as `build_table` does.
    if let Some(body) = table.bodies.first_mut()
        && !body.body_rows.is_empty()
    {
        let head = body.body_rows.remove(0);
        table.head.rows.push(head);
    }

    assert_eq!(
        image_audit(&doc(vec![Block::Table(Box::new(table))])),
        ImageAudit {
            total: 1,
            described: 0
        },
        "the header cell's image is counted, and counted as undescribed"
    );
}
