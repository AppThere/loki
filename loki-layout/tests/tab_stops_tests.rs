// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-geometry assertions for tab-stop alignment and leaders.
//!
//! GPU-free tests that lay out a paragraph containing a tab and a single
//! explicit tab stop, then assert where the post-tab content lands relative to
//! the stop (left/right/center/decimal) and that leaders are drawn. Guards the
//! Word contract for TOC dot-leaders and decimal-aligned number columns.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::units::Points;

use loki_layout::{
    FlowOutput, FontResources, LayoutMode, LayoutOptions, PositionedItem, flow_section,
};

const STOP: f64 = 300.0;

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn wide_layout() -> PageLayout {
    PageLayout {
        page_size: PageSize {
            width: Points::new(420.0),
            height: Points::new(400.0),
        },
        margins: PageMargins {
            top: Points::new(10.0),
            bottom: Points::new(10.0),
            left: Points::new(0.0),
            right: Points::new(10.0),
            ..PageMargins::default()
        },
        ..PageLayout::default()
    }
}

fn tab_para(text: &str, alignment: TabAlignment, leader: TabLeader) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            tab_stops: Some(vec![TabStop {
                position: Points::new(STOP),
                alignment,
                leader,
            }]),
            ..Default::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    }
}

fn layout(para: StyledParagraph) -> Vec<PositionedItem> {
    let mut r = test_resources();
    let section = Section::with_layout_and_blocks(wide_layout(), vec![Block::StyledPara(para)]);
    match flow_section(
        &mut r,
        &section,
        &StyleCatalog::new(),
        &LayoutMode::Pageless,
        1.0,
        &LayoutOptions::default(),
        &[],
    ) {
        FlowOutput::Canvas { items, .. } => items,
        _ => panic!("expected Canvas output"),
    }
}

/// `(origin_x, right_edge)` of the rightmost glyph run — the content after the
/// tab. `right_edge = origin.x + sum(glyph advances)`.
fn post_tab_run(items: &[PositionedItem]) -> (f32, f32) {
    items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(run) => {
                let adv: f32 = run.glyphs.iter().map(|g| g.advance).sum();
                Some((run.origin.x, run.origin.x + adv))
            }
            _ => None,
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .expect("a post-tab glyph run")
}

#[test]
fn right_tab_aligns_content_right_edge_to_stop() {
    let items = layout(tab_para("Ch\t12", TabAlignment::Right, TabLeader::None));
    let (origin, right) = post_tab_run(&items);
    assert!(
        (right - STOP as f32).abs() < 2.0,
        "right-tab content must end at the stop ({STOP}); right edge = {right}"
    );
    assert!(origin < STOP as f32, "content must start left of the stop");
}

#[test]
fn center_tab_centers_content_on_stop() {
    let items = layout(tab_para("Ch\t12", TabAlignment::Center, TabLeader::None));
    let (origin, right) = post_tab_run(&items);
    let center = (origin + right) / 2.0;
    assert!(
        (center - STOP as f32).abs() < 2.0,
        "center-tab content must be centered on the stop ({STOP}); center = {center}"
    );
}

#[test]
fn decimal_tab_aligns_numbers_on_the_decimal_point() {
    // Two numbers with the same fractional suffix (".5"): a decimal stop puts
    // the '.' at the stop, so both right edges land at stop + width(".5") and
    // are therefore equal AND strictly right of the stop (distinguishing decimal
    // from right-alignment, which would end exactly at the stop).
    let (_, right_short) = post_tab_run(&layout(tab_para(
        "x\t1.5",
        TabAlignment::Decimal,
        TabLeader::None,
    )));
    let (_, right_long) = post_tab_run(&layout(tab_para(
        "x\t1234.5",
        TabAlignment::Decimal,
        TabLeader::None,
    )));
    assert!(
        (right_short - right_long).abs() < 1.0,
        "decimal-aligned numbers sharing a '.5' suffix must share a right edge: \
         {right_short} vs {right_long}"
    );
    assert!(
        right_short > STOP as f32 + 0.5,
        "the fractional part must extend right of the decimal stop ({STOP})"
    );
}

#[test]
fn dot_leader_fills_the_tab_gap() {
    let items = layout(tab_para("Ch\t12", TabAlignment::Right, TabLeader::Dot));
    let dots: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::FilledRect(r) => Some(r.rect.x()),
            _ => None,
        })
        .filter(|&x| x > 20.0 && x < STOP as f32)
        .collect();
    assert!(
        dots.len() >= 5,
        "a dot leader must place multiple dots in the gap before the stop; got {}",
        dots.len()
    );
}

#[test]
fn left_tab_unchanged_advances_to_stop() {
    // Regression: a plain left tab still advances content to begin at the stop.
    let items = layout(tab_para("Ch\tX", TabAlignment::Left, TabLeader::None));
    let (origin, _) = post_tab_run(&items);
    assert!(
        (origin - STOP as f32).abs() < 2.0,
        "left-tab content must begin at the stop ({STOP}); origin = {origin}"
    );
}
