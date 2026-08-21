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
    // Same page geometry as s0 — a *size-changing* continuous break is promoted
    // to a page break (Word fidelity), which is not what this test exercises.
    s1.layout.page_size.height = Points::new(220.0);
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

/// Word fidelity: a `continuous` section break that changes the page size is
/// promoted to a page break — the new section starts its own page carrying its
/// own geometry (an A4 continuous section after Letter must not be laid out on
/// the Letter page). Regression test for the ACID document's missing page.
#[test]
fn continuous_break_with_new_page_size_starts_its_own_page() {
    use loki_doc_model::layout::{PageLayout, PageSize, SectionStart};

    let letter_layout = PageLayout {
        page_size: PageSize::letter(),
        ..PageLayout::default()
    };
    let a4_layout = PageLayout {
        page_size: PageSize::a4(),
        ..PageLayout::default()
    };

    let mut s1 = section(&["letter body"]);
    s1.layout = letter_layout;
    let mut s2 = section(&["a4 body"]);
    s2.layout = a4_layout;
    s2.start = SectionStart::Continuous;

    let mut doc = Document::new();
    doc.sections = vec![s1, s2];

    let mut r = resources();
    let DocumentLayout::Paginated(paginated) = layout_document(
        &mut r,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    assert!(
        paginated.pages.len() >= 2,
        "the size-changing continuous section must start a new page (got {})",
        paginated.pages.len()
    );
    let last = paginated.pages.last().expect("at least one page");
    assert!(
        (last.page_size.width - 595.28).abs() < 0.5 && (last.page_size.height - 841.89).abs() < 0.5,
        "the new page must carry the A4 geometry, got {:?}",
        last.page_size
    );
}

/// The counterpart: a genuine continuous section (same page size) still shares
/// the previous section's page — the Word-fidelity exception must not regress
/// ordinary continuous column changes.
#[test]
fn continuous_break_with_same_page_size_still_shares_the_page() {
    use loki_doc_model::layout::SectionStart;

    let s1 = section(&["intro"]);
    let mut s2 = section(&["continues on the same page"]);
    s2.start = SectionStart::Continuous;

    let mut doc = Document::new();
    doc.sections = vec![s1, s2];

    let mut r = resources();
    let DocumentLayout::Paginated(paginated) = layout_document(
        &mut r,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) else {
        panic!("Paginated mode must yield a Paginated layout");
    };
    assert_eq!(
        paginated.pages.len(),
        1,
        "a same-geometry continuous section shares the page"
    );
}

/// A `nextPage` section start collapses its first paragraph's `space_before`
/// against the **previous section's** trailing `space_after`.
///
/// Measured on Word 16.0 — three documents, each a `nextPage` section break
/// whose following paragraph requests 36pt `before`, differing only in the
/// `after` on the paragraph carrying the break. First-baselines, read against
/// the same document with `before = 0`:
///
/// | break paragraph's `after` | applied |
/// |---------------------------|---------|
/// | 0 pt                      | 36.00pt |
/// | 8 pt                      | 27.96pt |
/// | 20 pt                     | 15.96pt |
///
/// i.e. `max(0, before - after)` — the same collapse Word applies mid-page and
/// across a `w:pageBreakBefore`, not the full `before`.
///
/// Each section is its own page sequence with its own `FlowState`, so the value
/// has to be threaded across that boundary (`flow_section_group`'s `carry`).
/// Nothing inside one section can observe the previous one.
#[test]
fn a_section_start_collapses_space_before_against_the_previous_sections_space_after() {
    use loki_doc_model::style::props::para_props::{ParaProps, Spacing};
    use loki_primitives::units::Points;

    /// First baseline of section two's first paragraph, given the `after` on
    /// section one's last paragraph and the `before` on section two's first.
    fn head_baseline(after: f64, before: f64) -> f32 {
        let mut s1 = section(&["tail of section one"]);
        let Block::StyledPara(p) = &mut s1.blocks[0] else {
            panic!("a paragraph")
        };
        p.direct_para_props = Some(Box::new(ParaProps {
            space_after: Some(Spacing::Exact(Points::new(after))),
            ..Default::default()
        }));

        let mut s2 = section(&["head of section two"]);
        s2.start = loki_doc_model::layout::SectionStart::NewPage;
        let Block::StyledPara(p) = &mut s2.blocks[0] else {
            panic!("a paragraph")
        };
        p.direct_para_props = Some(Box::new(ParaProps {
            space_before: Some(Spacing::Exact(Points::new(before))),
            ..Default::default()
        }));

        let mut doc = Document::default();
        doc.sections = vec![s1, s2];
        let mut r = resources();
        let DocumentLayout::Paginated(layout) = layout_document(
            &mut r,
            &doc,
            LayoutMode::Paginated,
            1.0,
            &LayoutOptions {
                preserve_for_editing: true,
                ..LayoutOptions::default()
            },
        ) else {
            panic!("paginated")
        };
        assert_eq!(
            layout.pages.len(),
            2,
            "the section break makes a second page"
        );
        let page = &layout.pages[1];
        let ed = page.editing_data.as_ref().expect("editing data");
        let para = ed.paragraphs.first().expect("a paragraph on page two");
        page.margins.top + para.origin.1 + para.layout.first_baseline
    }

    // The control: no space_before at all.
    let none = head_baseline(0.0, 0.0);

    // Nothing to collapse against — the whole 36pt applies.
    let full = head_baseline(0.0, 36.0);
    assert!(
        (full - none - 36.0).abs() < 0.01,
        "with no preceding space_after the whole space_before applies: \
         {none} vs {full}"
    );

    // 8pt of it is already spent at the foot of the previous page.
    let collapsed = head_baseline(8.0, 36.0);
    assert!(
        (collapsed - none - 28.0).abs() < 0.01,
        "36pt before behind an 8pt after must apply 28pt: {none} vs {collapsed}"
    );

    // The inversion: a larger `after` collapses more, so this cannot pass on a
    // layout that subtracts a fixed amount.
    let more = head_baseline(20.0, 36.0);
    assert!(
        (more - none - 16.0).abs() < 0.01,
        "36pt before behind a 20pt after must apply 16pt: {none} vs {more}"
    );

    // …and one at least as large as `before` swallows it entirely, rather than
    // pushing the paragraph *above* the top margin.
    let swallowed = head_baseline(40.0, 36.0);
    assert!(
        (swallowed - none).abs() < 0.01,
        "an after larger than before must swallow it whole: {none} vs {swallowed}"
    );
}
