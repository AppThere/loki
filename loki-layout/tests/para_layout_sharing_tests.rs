// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 09 S9-1: the shaping cache and the page editing index share **one**
//! `ParagraphLayout` allocation.
//!
//! Before S9-1, a cache hit cloned the whole layout out of `ParaCache` and
//! `place_paragraph_layout` cloned it a second time for `PageParagraphData`, so
//! every placement of a paragraph carried its own deep copy of the glyph items
//! and both byte-index maps — measured at 39.3 B/char of per-placement editing
//! residency (`docs/spikes/S09.0-layout-residency-census.md` §10b, §10c).
//!
//! The sharing is invisible to every behavioural test: identical output, fewer
//! allocations. So it needs a test that asserts *identity* rather than equality,
//! or the next refactor to reintroduce a clone will pass the whole suite.
//! `Arc::ptr_eq` is that assertion.
//!
//! The copy-on-write half matters just as much. Inline images, floats, and
//! picture bullets are injected after shaping, so those paragraphs must take a
//! private copy — sharing them would leak one paragraph's image into every other
//! placement of the same text.

use std::sync::Arc;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{Inline, LinkTarget};
use loki_doc_model::document::Document;
use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout, layout_document,
};

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

fn para(text: &str) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn lay_out(doc: &Document) -> PaginatedLayout {
    let mut r = resources();
    match layout_document(
        &mut r,
        doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
            ..Default::default()
        },
    ) {
        DocumentLayout::Paginated(p) => p,
        other => panic!("paginated mode returned {other:?}"),
    }
}

/// Every editing-index layout, in placement order.
fn editing_layouts(pages: &PaginatedLayout) -> Vec<Arc<loki_layout::ParagraphLayout>> {
    pages
        .pages
        .iter()
        .flat_map(|p| p.editing_data.iter())
        .flat_map(|e| e.paragraphs.iter())
        .map(|p| Arc::clone(&p.layout))
        .collect()
}

/// Every editing entry for byte-identical paragraph content must be the *same*
/// allocation, not an equal copy. This is S9-1's whole point, and it is what the
/// duplication sweep measures as the per-placement coefficient `P`.
#[test]
fn identical_paragraphs_share_one_editing_layout() {
    let mut doc = Document::new();
    doc.sections[0].blocks = (0..6).map(|_| para("Repeated boilerplate line")).collect();

    let layouts = editing_layouts(&lay_out(&doc));

    assert_eq!(layouts.len(), 6, "one editing entry per placed paragraph");
    for (i, l) in layouts.iter().enumerate().skip(1) {
        assert!(
            Arc::ptr_eq(&layouts[0], l),
            "placement {i} holds its own copy of an identical paragraph's layout — \
             the cache and the editing index are no longer sharing one allocation (S9-1)"
        );
    }
}

/// Different content must not be conflated. The sharing is keyed on the cache
/// key, so this is really a check that the key still distinguishes text — a
/// failure here would be a correctness bug, not a residency one.
#[test]
fn different_paragraphs_do_not_share() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("first distinct line"), para("second distinct line")];

    let layouts = editing_layouts(&lay_out(&doc));

    assert_eq!(layouts.len(), 2);
    assert!(
        !Arc::ptr_eq(&layouts[0], &layouts[1]),
        "distinct paragraph content shared one layout — the cache key is wrong"
    );
}

/// Copy-on-write: a paragraph the flow mutates after shaping must take a private
/// copy, or the injected item leaks into every other placement of the same text.
///
/// This is the hazard S9-1 introduces. Sharing is correct only for the layout as
/// shaped; inline images, floats and picture bullets are pushed into `items`
/// afterwards, so those paragraphs must diverge from the cache entry. Here two
/// paragraphs carry identical text and only one carries an image — if they came
/// back sharing one allocation, the plain paragraph would render the image too.
#[test]
fn a_mutated_paragraph_does_not_share_with_its_plain_twin() {
    let text = "Identical caption text";
    let with_image = Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![
            Inline::Str(text.into()),
            Inline::Image(
                NodeAttr::default(),
                vec![],
                LinkTarget {
                    url: "test-image.png".into(),
                    title: None,
                },
            ),
        ],
        attr: NodeAttr::default(),
    });

    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para(text), with_image];

    let layouts = editing_layouts(&lay_out(&doc));
    assert_eq!(layouts.len(), 2);
    assert!(
        !Arc::ptr_eq(&layouts[0], &layouts[1]),
        "the image-bearing paragraph shares the plain paragraph's layout — \
         the copy-on-write guard in flow_paragraph is not firing, so the image \
         would appear on both"
    );
}
