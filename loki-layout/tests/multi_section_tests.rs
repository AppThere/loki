// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Multi-section layout: block indices must be **global** (document order across
//! every section). The editor and the `loro_mutation` layer address blocks by a
//! single flat index, so a hit-test / cursor position must resolve to the right
//! section's block.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::Section;

use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document};

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

fn section(texts: &[&str]) -> Section {
    let mut s = Section::new();
    for t in texts {
        s.blocks.push(Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str((*t).into())],
            attr: NodeAttr::default(),
        }));
    }
    s
}

#[test]
fn block_index_is_global_across_sections() {
    let mut doc = Document::new();
    doc.sections = vec![section(&["a0", "a1"]), section(&["b0", "b1", "b2"])];

    let mut r = resources();
    let layout = layout_document(
        &mut r,
        &doc,
        LayoutMode::Reflow {
            available_width: 600.0,
        },
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
            ..Default::default()
        },
    );

    let DocumentLayout::Continuous(cl) = layout else {
        panic!("Reflow mode must yield a Continuous layout");
    };

    // 2 blocks in section 0 (global indices 0, 1), 3 in section 1 (global 2, 3,
    // 4). Without the global offset these would be [0, 1, 0, 1, 2] and section-1
    // edits would hit section 0.
    let indices: Vec<usize> = cl.paragraphs.iter().map(|p| p.block_index).collect();
    assert_eq!(
        indices,
        vec![0, 1, 2, 3, 4],
        "block indices must be global (cumulative) across sections"
    );
}

#[test]
fn continuous_section_shares_previous_page() {
    use loki_doc_model::layout::SectionStart;
    use loki_layout::PaginatedLayout;

    let s0 = section(&["Intro paragraph that ends section zero."]);
    let mut s1 = section(&["Continuation paragraph one.", "Continuation paragraph two."]);

    let mut r = resources();
    let opts = LayoutOptions::default();

    // Continuous: section 1's short content packs onto section 0's last page.
    s1.start = SectionStart::Continuous;
    let mut doc = Document::new();
    doc.sections = vec![s0.clone(), s1.clone()];
    let DocumentLayout::Paginated(PaginatedLayout { pages, .. }) =
        layout_document(&mut r, &doc, LayoutMode::Paginated, 1.0, &opts)
    else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    assert_eq!(
        pages.len(),
        1,
        "a continuous section must share the previous section's page, not start a new one"
    );

    // Control: the SAME content as a nextPage section starts a new page → 2 pages.
    s1.start = SectionStart::NewPage;
    doc.sections = vec![s0, s1];
    let DocumentLayout::Paginated(PaginatedLayout { pages: pages2, .. }) =
        layout_document(&mut r, &doc, LayoutMode::Paginated, 1.0, &opts)
    else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    assert_eq!(
        pages2.len(),
        2,
        "a nextPage section must start a fresh page"
    );
}

#[test]
fn odd_page_section_inserts_a_blank_filler_when_parity_is_wrong() {
    use loki_doc_model::layout::SectionStart;
    use loki_layout::PaginatedLayout;

    // Section 0 fills exactly page 1 (odd). An oddPage section 1 would then start
    // on page 2 (even) — so a blank filler page is inserted, and section 1 lands
    // on page 3.
    let s0 = section(&["Section zero, one page."]);
    let mut s1 = section(&["Section one starts on an odd page."]);
    s1.start = SectionStart::OddPage;

    let mut r = resources();
    let opts = LayoutOptions::default();
    let mut doc = Document::new();
    doc.sections = vec![s0.clone(), s1.clone()];
    let DocumentLayout::Paginated(PaginatedLayout { pages, .. }) =
        layout_document(&mut r, &doc, LayoutMode::Paginated, 1.0, &opts)
    else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    assert_eq!(
        pages.len(),
        3,
        "an oddPage break inserts one blank filler page"
    );
    assert!(
        pages[1].content_items.is_empty(),
        "the middle page is the blank filler"
    );
    assert!(
        !pages[0].content_items.is_empty() && !pages[2].content_items.is_empty(),
        "the real section pages carry content"
    );

    // Control: an evenPage section after the same single page starts on page 2
    // (even) already — no filler, just 2 pages.
    s1.start = SectionStart::EvenPage;
    doc.sections = vec![s0, s1];
    let DocumentLayout::Paginated(PaginatedLayout { pages: even, .. }) =
        layout_document(&mut r, &doc, LayoutMode::Paginated, 1.0, &opts)
    else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    assert_eq!(
        even.len(),
        2,
        "an evenPage section already on the right parity needs no filler"
    );
}

#[test]
fn continuous_multi_column_section_flows_into_two_columns_on_shared_page() {
    use loki_doc_model::layout::SectionStart;
    use loki_doc_model::layout::page::SectionColumns;
    use loki_layout::{PaginatedLayout, PositionedItem};
    use loki_primitives::units::Points;

    let mut s0 = section(&["Single column intro at the top of the page."]);
    // A short page so the continuous section's content overflows column 0 into
    // column 1 (the column flow fills top-to-bottom, fill-first). Keep the
    // default (Letter) width so the second column sits well to the right.
    s0.layout.page_size.height = Points::new(220.0);

    // Enough paragraphs to overflow the short first column.
    let lines: Vec<&str> = (0..14)
        .map(|_| "Column flow lorem ipsum dolor sit amet consectetur adipiscing.")
        .collect();
    let mut s1 = section(&lines);
    s1.start = SectionStart::Continuous;
    s1.layout.columns = Some(SectionColumns {
        count: 2,
        gap: Points::new(18.0),
        separator: false,
        widths: Vec::new(),
    });

    let mut doc = Document::new();
    doc.sections = vec![s0, s1];
    let mut r = resources();
    let DocumentLayout::Paginated(PaginatedLayout { pages, .. }) = layout_document(
        &mut r,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    // The first page is shared: it carries the single-column intro AND the start
    // of the two-column continuous section below it.
    let first = &pages[0];
    let left_edges: Vec<f32> = first
        .all_items()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some(g.origin.x),
            _ => None,
        })
        .collect();
    // Single-column intro near the left edge…
    assert!(
        left_edges.iter().any(|&x| x < 60.0),
        "expected single-column content near the left edge"
    );
    // …and second-column content well to the right — proof the continuous section
    // flowed into two columns on the SAME page as the intro above it.
    let max_x = left_edges.iter().cloned().fold(0.0_f32, f32::max);
    assert!(
        max_x > 200.0,
        "expected a second-column glyph run far from the left edge, got max x = {max_x}"
    );
}
