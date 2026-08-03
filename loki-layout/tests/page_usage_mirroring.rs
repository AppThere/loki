// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `style:page-usage="mirrored"` must reach the paginator (Spec 08 T6.1).
//!
//! # The defect this closes is a field that round-trips and renders nothing
//!
//! Mirrored margins were wired end-to-end for **OOXML** — `w:mirrorMargins` in
//! `settings.xml` → `DocumentSettings` → `LayoutOptions` → `mirrored_margins()`.
//! ODF states the same property per page layout, and neither the reader nor the
//! writer knew about it, so an ODT that mirrors laid out single-sided.
//!
//! Adding the model field and its codec is not the fix on its own: a property
//! that survives a round-trip and changes no pixel is reachable-but-unimplemented,
//! which this project treats as a defect rather than as partial progress. This
//! test is the half that says the value arrives somewhere it matters.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageUsage};
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_primitives::units::Points;

fn resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
            break;
        }
    }
    r
}

/// Asymmetric left/right margins, so a swap is visible. Equal margins would
/// make every assertion below pass whether or not anything mirrored — the
/// fixture has to be able to express the defect (L08-044).
const LEFT: f64 = 144.0;
const RIGHT: f64 = 36.0;

fn doc_with(usage: PageUsage) -> Document {
    let mut section = Section::new();
    section.layout = PageLayout {
        page_usage: usage,
        margins: PageMargins {
            left: Points::new(LEFT),
            right: Points::new(RIGHT),
            ..PageMargins::default()
        },
        ..PageLayout::default()
    };
    // Enough content to run past one page, or there is no even page to check.
    for i in 0..120 {
        section.blocks.push(Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str(format!(
                "Paragraph number {i} of the body text."
            ))],
            attr: NodeAttr::default(),
        }));
    }
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

fn margins_per_page(usage: PageUsage) -> Vec<(f64, f64)> {
    let mut r = resources();
    let layout = layout_document(
        &mut r,
        &doc_with(usage),
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    );
    let DocumentLayout::Paginated(p) = layout else {
        panic!("paginated mode must return a paginated layout");
    };
    assert!(p.pages.len() >= 2, "fixture must span at least two pages");
    p.pages
        .iter()
        .map(|pg| (f64::from(pg.margins.left), f64::from(pg.margins.right)))
        .collect()
}

/// **A mirrored document swaps left and right on even pages, and only there.**
#[test]
fn mirrored_page_usage_alternates_margins_from_the_odf_property_alone() {
    let pages = margins_per_page(PageUsage::Mirrored);
    for (i, (left, right)) in pages.iter().enumerate() {
        let page_number = i + 1;
        let (want_l, want_r) = if page_number % 2 == 0 {
            (RIGHT, LEFT)
        } else {
            (LEFT, RIGHT)
        };
        assert!(
            (left - want_l).abs() < 0.01 && (right - want_r).abs() < 0.01,
            "page {page_number}: got ({left}, {right}), wanted ({want_l}, {want_r})",
        );
    }
}

/// **The polarity.** Without it every assertion above passes for a paginator
/// that mirrors unconditionally — which would silently swap the margins of every
/// single-sided document in the suite.
#[test]
fn an_unmirrored_document_keeps_the_same_margins_on_every_page() {
    for usage in [PageUsage::All, PageUsage::Left, PageUsage::Right] {
        for (i, (left, right)) in margins_per_page(usage).iter().enumerate() {
            assert!(
                (left - LEFT).abs() < 0.01 && (right - RIGHT).abs() < 0.01,
                "{usage:?} page {}: got ({left}, {right}), wanted ({LEFT}, {RIGHT})",
                i + 1,
            );
        }
    }
}
