// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! First-page / even-page header and footer selection (Spec 08 T6.6).
//!
//! The model, both readers, both writers and the Loro bridge all carried
//! `header_first` / `header_even` before these tests existed; the paginator's
//! *selection* — the half that makes them a feature rather than a field — had
//! no test in this crate at all, and that is where the defect was.
//!
//! # Reading a variant back without text
//!
//! `PositionedGlyphRun` carries glyph ids, not the string that produced them,
//! so a variant is identified by **glyph count**: each fixture gives its four
//! variants distinct run lengths. That is enough to say *which* variant landed
//! on a page, which is the only question these tests ask.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::Section;
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::PageLayout;

use loki_layout::items::PositionedItem;
use loki_layout::result::LayoutPage;
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

fn para(t: &str) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(t.into())],
        attr: NodeAttr::default(),
    })
}

/// A header/footer whose glyph count is `n` — the label this suite reads back.
fn hf(kind: HeaderFooterKind, n: usize) -> HeaderFooter {
    HeaderFooter {
        kind,
        blocks: vec![para(&"M".repeat(n))],
    }
}

fn header_glyphs(page: &LayoutPage) -> usize {
    page.header_items
        .iter()
        .map(|i| match i {
            PositionedItem::GlyphRun(g) => g.glyphs.len(),
            _ => 0,
        })
        .sum()
}

fn footer_glyphs(page: &LayoutPage) -> usize {
    page.footer_items
        .iter()
        .map(|i| match i {
            PositionedItem::GlyphRun(g) => g.glyphs.len(),
            _ => 0,
        })
        .sum()
}

/// A section of `paras` paragraphs whose header variants are `first`/`even`/
/// `default` glyphs long.
fn section(paras: usize, tag: &str, first: usize, even: usize, default: usize) -> Section {
    let mut s = Section {
        layout: PageLayout::default(),
        ..Section::new()
    };
    for i in 0..paras {
        s.blocks.push(para(&format!(
            "{tag} paragraph {i} with enough words to wrap across the page width \
             and occupy a couple of lines of body content."
        )));
    }
    s.layout.header_first = Some(hf(HeaderFooterKind::First, first));
    s.layout.header_even = Some(hf(HeaderFooterKind::Even, even));
    s.layout.header = Some(hf(HeaderFooterKind::Default, default));
    s
}

fn paginate(doc: &Document) -> Vec<std::sync::Arc<LayoutPage>> {
    let mut r = resources();
    let DocumentLayout::Paginated(p) = layout_document(
        &mut r,
        doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) else {
        panic!("expected a paginated layout");
    };
    p.pages.to_vec()
}

/// **The first-page variant belongs to the first page of its *section*.**
///
/// `select` asked `pn == 1`, the document-global page number, so only section 1
/// could ever show a first-page header: every later section's `header_first`
/// was imported, stored in the CRDT and written back out, and never rendered.
/// The correct quantity was being computed fourteen lines below for the
/// `w:pgNumType` restart and left unread here.
#[test]
fn each_section_shows_its_own_first_page_header() {
    let mut doc = Document::new();
    doc.sections = vec![
        section(4, "S0", 3, 4, 5),
        section(4, "S1", 6, 7, 8),
        section(4, "S2", 9, 10, 11),
    ];
    let pages = paginate(&doc);

    // Each section is short enough to be exactly one page, so page N is section
    // N-1's first page. Asserted, not assumed: if a section ever spilled, the
    // mapping below would be reading the wrong pages and still find headers.
    assert_eq!(
        pages.len(),
        3,
        "fixture should be one page per section, got {} pages",
        pages.len()
    );

    // Section 1's start is page 2 and section 2's is page 3 — neither is page 1,
    // which is the whole point: the old rule could only ever fire on page 1.
    for (i, want) in [3usize, 6, 9].iter().enumerate() {
        let page = &pages[i];
        assert_eq!(page.page_number, i + 1, "page ordering changed");
        assert_eq!(
            header_glyphs(page),
            *want,
            "section {i} starts on page {} and did not show its first-page \
             header (got {} glyphs, wanted {want})",
            page.page_number,
            header_glyphs(page)
        );
    }
}

/// The inverse: a section **without** a first-page variant must fall through to
/// the default on its own first page, rather than borrowing another section's.
///
/// Without this, `is_first` could be hardwired true and the test above would
/// still pass.
#[test]
fn a_section_with_no_first_page_variant_falls_through() {
    let mut doc = Document::new();
    let mut s0 = section(4, "S0", 3, 4, 5);
    let mut s1 = section(4, "S1", 6, 7, 8);
    s1.layout.header_first = None;
    s0.layout.header_first = None;
    doc.sections = vec![s0, s1];
    let pages = paginate(&doc);

    assert!(pages.len() >= 2, "fixture needs at least two pages");
    // Page 1 is odd → default (5); page 2 is even → even variant (7 for
    // section 1, whose page it is).
    assert_eq!(
        header_glyphs(&pages[0]),
        5,
        "page 1 without a first-page variant should show the default header"
    );
}

/// **The even variant still applies on a section's first page when that page is
/// even.** First-page and even-page are independent questions, and making the
/// first-page fix section-scoped must not turn every section start into an odd
/// page by fiat.
#[test]
fn an_even_numbered_section_start_without_a_first_variant_is_still_even() {
    let mut doc = Document::new();
    let s0 = section(4, "S0", 3, 4, 5);
    let mut s1 = section(4, "S1", 6, 7, 8);
    s1.layout.header_first = None;
    doc.sections = vec![s0, s1];
    let pages = paginate(&doc);

    assert!(pages.len() >= 2, "fixture needs at least two pages");
    let p2 = &pages[1];
    assert_eq!(p2.page_number, 2, "second page should be numbered 2");
    assert_eq!(
        header_glyphs(p2),
        7,
        "an even-numbered section start with no first-page variant should show \
         the even header"
    );
}

/// **The first-page variant applies to the section's first page and no other.**
///
/// Every other test here uses one-page sections, where every page *is* a section
/// start — so `is_first` is never false while a first-page variant exists, and
/// hardwiring it true would pass them all. This is the scenario that makes the
/// guard discriminate: a section spanning several pages, whose later pages must
/// fall through to default/even.
#[test]
fn the_first_page_variant_does_not_leak_onto_the_rest_of_the_section() {
    let mut doc = Document::new();
    doc.sections = vec![section(60, "S0", 3, 4, 5)];
    let pages = paginate(&doc);
    assert!(
        pages.len() >= 3,
        "fixture needs a multi-page section, got {} pages",
        pages.len()
    );

    assert_eq!(
        header_glyphs(&pages[0]),
        3,
        "page 1 should show the first-page header"
    );
    assert_eq!(
        header_glyphs(&pages[1]),
        4,
        "page 2 is even and not a section start — it should show the even header"
    );
    assert_eq!(
        header_glyphs(&pages[2]),
        5,
        "page 3 is odd and not a section start — it should show the default header"
    );
}

/// Footers select by the same rule as headers — they share `select`, but they
/// pass their own three variants to it, so a transposed argument would show up
/// here and nowhere else.
#[test]
fn footers_select_the_same_way_headers_do() {
    let mut doc = Document::new();
    let mut s0 = section(4, "S0", 3, 4, 5);
    let mut s1 = section(4, "S1", 6, 7, 8);
    for (s, base) in [(&mut s0, 12usize), (&mut s1, 15usize)] {
        s.layout.footer_first = Some(hf(HeaderFooterKind::First, base));
        s.layout.footer_even = Some(hf(HeaderFooterKind::Even, base + 1));
        s.layout.footer = Some(hf(HeaderFooterKind::Default, base + 2));
    }
    doc.sections = vec![s0, s1];
    let pages = paginate(&doc);

    assert!(pages.len() >= 2, "fixture needs at least two pages");
    assert_eq!(
        footer_glyphs(&pages[0]),
        12,
        "section 0's first page should show its first-page footer"
    );
    assert_eq!(
        footer_glyphs(&pages[1]),
        15,
        "section 1's first page should show its first-page footer"
    );
}
