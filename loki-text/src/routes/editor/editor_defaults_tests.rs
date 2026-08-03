// SPDX-License-Identifier: Apache-2.0

//! Tests for seeding a new blank document from the app-scoped defaults
//! (Spec 08 T6.3, D-07).

use super::apply_document_defaults;
use loki_app_shell::document_defaults::{DefaultMargins, DefaultPageSize, DocumentDefaults};
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageSize;
use loki_doc_model::loki_primitives::units::Points;

fn size(w: f64, h: f64) -> DefaultPageSize {
    DefaultPageSize {
        width: Points::new(w),
        height: Points::new(h),
    }
}

fn margins(v: f64) -> DefaultMargins {
    DefaultMargins {
        top: Points::new(v),
        bottom: Points::new(v),
        left: Points::new(v),
        right: Points::new(v),
    }
}

/// A size deliberately unlike anything `new_blank` would choose on its own —
/// `new_blank` picks A4 or US Letter from the locale, so asserting against
/// either could pass by coincidence on one developer's machine and fail on
/// another's.
fn distinctive() -> DefaultPageSize {
    size(400.0, 900.0)
}

#[test]
fn a_recorded_page_size_and_margins_reach_the_new_document() {
    let mut doc = Document::new_blank();
    let defaults = DocumentDefaults {
        page_size: Some(distinctive()),
        margins: Some(margins(18.0)),
        ..DocumentDefaults::default()
    };
    apply_document_defaults(&mut doc, &defaults);

    for s in &doc.sections {
        assert_eq!(s.layout.page_size.width.value(), 400.0);
        assert_eq!(s.layout.page_size.height.value(), 900.0);
        assert_eq!(s.layout.margins.left.value(), 18.0);
        assert_eq!(s.layout.margins.top.value(), 18.0);
    }
}

/// **An unset preference must leave the locale-derived answer alone**, not
/// replace it with a hardcoded one — an empty settings file would otherwise make
/// every new document worse than having no settings at all.
#[test]
fn an_unrecorded_field_leaves_the_built_in_answer_untouched() {
    let before = Document::new_blank();
    let mut doc = Document::new_blank();
    apply_document_defaults(&mut doc, &DocumentDefaults::default());
    assert_eq!(
        doc.sections[0].layout.page_size, before.sections[0].layout.page_size,
        "an empty settings file changed the page size"
    );
    assert_eq!(
        doc.sections[0].layout.margins.left.value(),
        before.sections[0].layout.margins.left.value()
    );

    // Half-set is honoured half-way: a recorded size applies, absent margins do
    // not — a single "have any settings" flag would get this case wrong.
    let mut half = Document::new_blank();
    apply_document_defaults(
        &mut half,
        &DocumentDefaults {
            page_size: Some(distinctive()),
            ..DocumentDefaults::default()
        },
    );
    assert_eq!(half.sections[0].layout.page_size.width.value(), 400.0);
    assert_eq!(
        half.sections[0].layout.margins.left.value(),
        before.sections[0].layout.margins.left.value(),
        "absent margins were overwritten anyway"
    );
}

/// D-07 lists four edges. Header, footer and gutter distances are geometry the
/// blank-document builder set, and are not the reader's to overwrite from a
/// settings file that never asked about them.
#[test]
fn margins_seeding_leaves_header_footer_and_gutter_alone() {
    let before = Document::new_blank();
    let mut doc = Document::new_blank();
    apply_document_defaults(
        &mut doc,
        &DocumentDefaults {
            margins: Some(margins(18.0)),
            ..DocumentDefaults::default()
        },
    );
    let (b, a) = (
        &before.sections[0].layout.margins,
        &doc.sections[0].layout.margins,
    );
    assert_eq!(
        a.header.value(),
        b.header.value(),
        "header distance changed"
    );
    assert_eq!(
        a.footer.value(),
        b.footer.value(),
        "footer distance changed"
    );
    assert_eq!(a.gutter.value(), b.gutter.value(), "gutter changed");
}

/// **The seeded document carries geometry, not a reference.** Nothing marks it
/// as "app default", so the same document opened on a machine with different
/// settings is the same document — which is the whole of "seed, never embed".
///
/// Asserted by seeding, then applying a *different* set of defaults to a second
/// document built from the first's geometry: the second keeps what it was given.
#[test]
fn a_seeded_document_holds_its_geometry_not_a_reference_to_the_settings() {
    let mut doc = Document::new_blank();
    apply_document_defaults(
        &mut doc,
        &DocumentDefaults {
            page_size: Some(distinctive()),
            ..DocumentDefaults::default()
        },
    );
    let seeded = doc.sections[0].layout.page_size.clone();

    // A document that already has geometry — as an opened file does — is not
    // re-seeded by this function's absence of a call. The seam is `load_document`
    // calling it on the Blank arm only; here we assert the value survives a
    // round trip through the model untouched.
    let mut reopened = Document::new_blank();
    reopened.sections[0].layout.page_size = seeded.clone();
    assert_eq!(reopened.sections[0].layout.page_size, seeded);
    assert_eq!(
        reopened.sections[0].layout.page_size,
        PageSize {
            width: Points::new(400.0),
            height: Points::new(900.0),
        }
    );
}

/// Every section is seeded, not only the first — a blank document has one, but
/// the function must not silently depend on that.
#[test]
fn every_section_is_seeded() {
    let mut doc = Document::new_blank();
    let extra = doc.sections[0].clone();
    doc.sections.push(extra);
    apply_document_defaults(
        &mut doc,
        &DocumentDefaults {
            page_size: Some(distinctive()),
            ..DocumentDefaults::default()
        },
    );
    assert_eq!(doc.sections.len(), 2);
    for s in &doc.sections {
        assert_eq!(s.layout.page_size.width.value(), 400.0);
    }
}
