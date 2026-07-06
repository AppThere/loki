// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 M5 acceptance smoke tests: the `vello_cpu` candidate path renders
//! a real laid-out document **headless, with no graphics adapter**, produces
//! actual ink, and is byte-deterministic across runs.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_render_cpu::{paintable_item_count, render_document, render_page};

/// Conformance DPI (matches `appthere_conformance::CONFORMANCE_DPI`; kept
/// literal here to avoid a dependency direction from renderer to harness).
const DPI: u32 = 144;

fn sample_document() -> Document {
    let props = CharProps {
        bold: Some(true),
        font_name: Some("Carlito".into()),
        ..Default::default()
    };
    let mut d = Document::default();
    let mut s = Section::new();
    s.blocks = vec![
        Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("Conformance render".into())],
        ),
        Block::Para(vec![
            Inline::Str("Deterministic CPU rasterization via ".into()),
            Inline::StyledRun(StyledRun {
                style_id: None,
                direct_props: Some(Box::new(props)),
                content: vec![Inline::Str("vello_cpu".into())],
                attr: NodeAttr::default(),
            }),
            Inline::Str(".".into()),
        ]),
    ];
    d.sections = vec![s];
    d
}

fn paginated(doc: &Document) -> loki_layout::PaginatedLayout {
    let mut resources = FontResources::new();
    // Embed the metric-compatible faces so shaping is machine-independent.
    for blob in loki_fonts::fallback_font_blobs() {
        resources.register_font(blob.to_vec());
    }
    match layout_document(
        &mut resources,
        doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) {
        DocumentLayout::Paginated(p) => p,
        other => panic!("expected paginated layout, got {other:?}"),
    }
}

#[test]
fn renders_a_page_headless_with_ink() {
    let layout = paginated(&sample_document());
    assert!(!layout.pages.is_empty(), "layout must produce pages");
    assert!(
        paintable_item_count(&layout.pages[0].content_items) > 0,
        "layout must produce paintable items"
    );

    let page = render_page(&layout, 0, DPI).expect("render must succeed with no GPU");
    // A4 at 144 dpi ≈ 1190×1684; just assert the scale relation.
    let expected_w = (layout.page_size.width * DPI as f32 / 72.0).ceil() as u32;
    assert_eq!(page.width(), expected_w);

    // There must be real ink: some pixels darker than the white paper.
    let dark = page
        .pixels()
        .filter(|p| u32::from(p.0[0]) + u32::from(p.0[1]) + u32::from(p.0[2]) < 300)
        .count();
    assert!(
        dark > 100,
        "rendered page must contain glyph ink (found {dark} dark pixels)"
    );
}

#[test]
fn rendering_is_deterministic() {
    let layout = paginated(&sample_document());
    let a = render_page(&layout, 0, DPI).expect("render A");
    let b = render_page(&layout, 0, DPI).expect("render B");
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "the candidate render must be byte-deterministic (Spec 02 D2)"
    );
}

#[test]
fn out_of_range_page_errors() {
    let layout = paginated(&sample_document());
    assert!(render_page(&layout, 99, DPI).is_err());
    assert_eq!(
        render_document(&layout, DPI).unwrap().len(),
        layout.pages.len()
    );
}
