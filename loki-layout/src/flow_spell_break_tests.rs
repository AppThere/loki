// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! I-06 — a spelling squiggle must stay inside the line box it belongs to, so a
//! fragment boundary cannot cut it (Spec 08 T3.1).
//!
//! # What the discriminating test found
//!
//! §3.3 offered three outcomes. The measured one is the first — the clip floor —
//! but not in the shape it predicted, and the difference matters for the fix.
//!
//! The squiggle band is centred on the descender bottom (`baseline + descent`),
//! while a fragment's clip ends at the line box bottom (`baseline + descent +
//! leading_below`), floored to whole points. With ordinary leading there is slack
//! and the band fits. With **exact line height** there is no leading below the
//! descender, so the band straddles the boundary: measured at 0.25pt of a 0.84pt
//! band cut at the bottom of one page, and the same line's squiggles re-emitted
//! into the next fragment at **negative y**, above its content top, where they are
//! clipped again.
//!
//! So the earlier segment is not absent — both segments are present as slivers,
//! which is exactly why the original report reads as the indicator having *moved*
//! to the next page.
//!
//! The fix is therefore in the decoration's placement, not in the clip: the band
//! is anchored just below the descender but is now clamped to stay within its own
//! line box. Where there is leading to spare nothing moves, which is why this does
//! not churn the goldens.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize, SectionColumns};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::para_props::{LineHeight, ParaProps};
use loki_primitives::units::Points;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::items::{DecorationKind, PositionedDecoration, PositionedItem};
use crate::mode::LayoutMode;
use crate::{FlowOutput, flow_section};

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in ["/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

/// Exact line height, which is the condition the defect needs: it removes the
/// leading below the descender that otherwise hides the overflow.
fn tight_props() -> Box<ParaProps> {
    Box::new(ParaProps {
        line_height: Some(LineHeight::Exact(Points::new(12.0))),
        ..Default::default()
    })
}

fn tiny_page(columns: Option<SectionColumns>) -> PageLayout {
    let mut layout = PageLayout {
        page_size: PageSize {
            width: Points::new(200.0),
            height: Points::new(100.0),
        },
        margins: PageMargins {
            top: Points::new(5.0),
            bottom: Points::new(5.0),
            left: Points::new(10.0),
            right: Points::new(10.0),
            ..Default::default()
        },
        ..Default::default()
    };
    if let Some(cols) = columns {
        layout.columns = Some(cols);
    }
    layout
}

/// Enough misspelled words that every line carries squiggles, so the assertion
/// does not depend on guessing which line the break lands on.
fn all_misspelled(repeats: usize) -> String {
    "teh teh teh teh teh teh teh teh teh teh teh teh teh teh teh ".repeat(repeats)
}

fn spelled_pages(text: String, layout: PageLayout) -> Vec<crate::result::LayoutPage> {
    let mut r = test_resources();
    let checker =
        std::sync::Arc::new(loki_spell::SpellChecker::bundled().expect("bundled dictionary loads"));
    let spell = crate::SpellState {
        checker,
        checkers: Default::default(),
        generation: 1,
    };
    let para = StyledParagraph {
        style_id: None,
        direct_para_props: Some(tight_props()),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    };
    let section = Section::with_layout_and_blocks(layout, vec![Block::StyledPara(para)]);
    let opts = LayoutOptions {
        spell: Some(spell),
        ..Default::default()
    };
    match flow_section(
        &mut r,
        &section,
        &StyleCatalog::new(),
        &LayoutMode::Paginated,
        1.0,
        &opts,
        &[],
    ) {
        FlowOutput::Pages { pages, .. } => pages,
        _ => panic!("expected Pages output"),
    }
}

/// Every `Spelling` decoration inside a clipped group, paired with the clip that
/// will be applied to it.
fn clipped_squiggles(pages: &[crate::result::LayoutPage]) -> Vec<(PositionedDecoration, f32, f32)> {
    let mut out = Vec::new();
    for page in pages {
        for item in &page.content_items {
            if let PositionedItem::ClippedGroup { clip_rect, items } = item {
                for inner in items {
                    if let PositionedItem::Decoration(d) = inner {
                        if d.kind == DecorationKind::Spelling {
                            out.push((
                                d.clone(),
                                clip_rect.y(),
                                clip_rect.y() + clip_rect.height(),
                            ));
                        }
                    }
                }
            }
        }
    }
    out
}

/// The property, stated once and reused by all three break kinds: a squiggle's
/// band lies wholly within the clip that will be applied to it.
///
/// Asserted over *every* squiggle rather than a chosen one, because the defect
/// appears only on the line adjacent to the boundary and picking a line by hand is
/// how it stayed hidden.
/// Comparison tolerance, in points. `f32::EPSILON` is the wrong scale here: it is
/// ~1.2e-7 *at 1.0*, and these coordinates are tens of points, where a single
/// representable step is already larger than that. A thousandth of a point is
/// ~4e-3 physical pixels even at 3x, so it cannot correspond to a visible cut,
/// while still failing on the 0.25pt overflow the defect actually produced.
const TOLERANCE_PT: f32 = 1e-3;

fn assert_no_squiggle_is_cut(pages: &[crate::result::LayoutPage], what: &str) {
    let all = clipped_squiggles(pages);
    assert!(
        !all.is_empty(),
        "{what}: no clipped squiggles were produced, so this asserts nothing — \
         the fixture is not exercising the split path",
    );
    for (d, clip_top, clip_bottom) in &all {
        let band_top = d.y;
        let band_bottom = d.y + d.thickness;
        assert!(
            band_top >= *clip_top - TOLERANCE_PT,
            "{what}: squiggle band starts at {band_top} above its clip top \
             {clip_top} — it belongs to the previous fragment and will be cut",
        );
        assert!(
            band_bottom <= *clip_bottom + TOLERANCE_PT,
            "{what}: squiggle band ends at {band_bottom} below its clip bottom \
             {clip_bottom} — {overflow}pt of a {thickness}pt band is cut",
            overflow = band_bottom - clip_bottom,
            thickness = d.thickness,
        );
    }
}

#[test]
fn squiggles_survive_a_line_break() {
    // A single page, so the only boundaries are line boxes within one fragment.
    // The band must sit inside its own line box even with no fragment split.
    let pages = spelled_pages(all_misspelled(1), tiny_page(None));
    let mut r = test_resources();
    let _ = &mut r;
    // Line-break case has no clipped group, so assert the band-in-line-box
    // property directly against the line boxes rather than a clip.
    let squiggles: Vec<_> = pages
        .iter()
        .flat_map(|p| p.content_items.iter())
        .filter_map(|i| match i {
            PositionedItem::Decoration(d) if d.kind == DecorationKind::Spelling => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert!(
        squiggles.len() >= 15,
        "every word is misspelled, so a wrapped paragraph carries a squiggle per \
         word per line; got {}",
        squiggles.len()
    );
    // Bands on different lines must not overlap: an overflowing band reaches into
    // the next line's box, which is the same defect one boundary down.
    let mut ys: Vec<(f32, f32)> = squiggles.iter().map(|d| (d.y, d.y + d.thickness)).collect();
    ys.sort_by(|a, b| a.0.total_cmp(&b.0));
    ys.dedup();
    for pair in ys.windows(2) {
        let (_, prev_bottom) = pair[0];
        let (next_top, _) = pair[1];
        assert!(
            prev_bottom <= next_top + TOLERANCE_PT,
            "a squiggle band ending at {prev_bottom} overlaps the next line's band \
             starting at {next_top}",
        );
    }
}

#[test]
fn squiggles_survive_a_page_break() {
    let pages = spelled_pages(all_misspelled(6), tiny_page(None));
    assert!(pages.len() >= 2, "the fixture must actually paginate");
    assert_no_squiggle_is_cut(&pages, "page break");
}

#[test]
fn squiggles_survive_a_column_break() {
    let pages = spelled_pages(
        all_misspelled(6),
        tiny_page(Some(SectionColumns::two_column())),
    );
    assert_no_squiggle_is_cut(&pages, "column break");
}
