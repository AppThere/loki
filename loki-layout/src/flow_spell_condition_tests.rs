// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! I-06's **condition space** — which line-height rules and font sizes put a
//! spelling squiggle across a fragment boundary (Spec 08 T3.1).
//!
//! # Why this sweep exists, and what it corrected
//!
//! The first pass at I-06 tested one configuration — 12pt Liberation Sans with
//! `LineHeight::Exact(12pt)` — reproduced there, did *not* reproduce with default
//! leading, and concluded the condition was "tight leading leaves no room below
//! the descender". **That was too narrow, and the sweep falsified it.**
//!
//! Measured against the pre-fix emitter, worst band overflow below the clip:
//!
//! | line height | 9pt | 11pt | 12pt | 14pt |
//! | --- | --- | --- | --- | --- |
//! | default | **0.473** | 0.000 | 0.000 | **0.844** |
//! | `Multiple(100/115/150%)` | 0.000 | 0.000 | 0.000 | 0.000 |
//! | `Exact(12pt)` / `Exact(14pt)` | 0.000 | 0.000 | **0.250** | **0.782** |
//! | `AtLeast(12pt)` / `AtLeast(14pt)` | **0.473** | 0.000 | 0.000 | **0.844** |
//!
//! So it reproduces with **default leading**, and at 14pt the loss is 0.844 of a
//! 0.844pt band — the squiggle disappears entirely. The governing quantity is not
//! "is there leading slack" but the *fractional phase* between accumulated line
//! height and the whole-point grid the clip is floored to: the band is lost when a
//! fragment's height lands just past a whole point, which depends on font size,
//! leading rule and how many lines precede the break.
//!
//! That phase dependence is what makes the original report's word "often" the
//! right one — it appears for some documents and sizes and not others, with no
//! pattern a user could infer. A "tight leading only" story would not have
//! predicted 14pt default leading, which is an ordinary configuration.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{LineHeight, ParaProps};
use loki_primitives::units::Points;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::items::{DecorationKind, PositionedItem};
use crate::mode::LayoutMode;
use crate::{FlowOutput, flow_section};

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

/// Worst overflow below the clip, in points, over every squiggle in a split
/// paragraph. `0.0` means nothing is cut.
fn worst_overflow(lh: Option<LineHeight>, size_pt: f32, family: Option<&str>) -> f32 {
    let pages = split_pages(lh, size_pt, family);
    let mut worst = 0.0_f32;
    for page in &pages {
        for item in &page.content_items {
            if let PositionedItem::ClippedGroup { clip_rect, items } = item {
                let bottom = clip_rect.y() + clip_rect.height();
                for inner in items {
                    if let PositionedItem::Decoration(d) = inner {
                        if d.kind == DecorationKind::Spelling {
                            worst = worst.max(d.y + d.thickness - bottom);
                        }
                    }
                }
            }
        }
    }
    worst.max(0.0)
}

/// Lays out the sweep fixture and returns its pages.
fn split_pages(
    lh: Option<LineHeight>,
    size_pt: f32,
    family: Option<&str>,
) -> Vec<crate::result::LayoutPage> {
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
        direct_para_props: Some(Box::new(ParaProps {
            line_height: lh,
            ..Default::default()
        })),
        direct_char_props: Some(Box::new(CharProps {
            font_size: Some(Points::new(f64::from(size_pt))),
            font_name: family.map(std::string::ToString::to_string),
            ..Default::default()
        })),
        inlines: vec![Inline::Str(
            "teh teh teh teh teh teh teh teh teh teh teh teh teh teh teh "
                .repeat(8)
                .into(),
        )],
        attr: NodeAttr::default(),
    };
    let layout = PageLayout {
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
    let section = Section::with_layout_and_blocks(layout, vec![Block::StyledPara(para)]);
    let opts = LayoutOptions {
        spell: Some(spell),
        ..Default::default()
    };
    let FlowOutput::Pages { pages, .. } = flow_section(
        &mut r,
        &section,
        &StyleCatalog::new(),
        &LayoutMode::Paginated,
        1.0,
        &opts,
        &[],
    ) else {
        panic!("expected Pages");
    };
    pages
}

/// Squiggles lying within one point of a fragment clip's bottom edge — the ones
/// the floor can reach. Zero means the fixture is not exercising the boundary,
/// whatever the overflow numbers say.
fn boundary_adjacent_squiggle_count(lh: Option<LineHeight>, size_pt: f32) -> usize {
    let pages = split_pages(lh, size_pt, None);
    let mut n = 0;
    for page in &pages {
        for item in &page.content_items {
            if let PositionedItem::ClippedGroup { clip_rect, items } = item {
                let bottom = clip_rect.y() + clip_rect.height();
                for inner in items {
                    if let PositionedItem::Decoration(d) = inner {
                        if d.kind == DecorationKind::Spelling
                            && (bottom - (d.y + d.thickness)).abs() <= 1.0
                        {
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    n
}

/// The property across the condition space, not at one point in it.
///
/// Every combination is asserted rather than the interesting ones, because the
/// interesting ones are exactly what the first pass got wrong: 12pt default
/// leading is clean and 14pt default leading loses the whole band, and nothing
/// about the mechanism says which is which without measuring.
#[test]
fn no_line_height_rule_or_font_size_cuts_a_squiggle() {
    for (label, lh) in [
        ("default", None),
        ("Multiple(100%)", Some(LineHeight::Multiple(100.0))),
        ("Multiple(115%)", Some(LineHeight::Multiple(115.0))),
        ("Multiple(150%)", Some(LineHeight::Multiple(150.0))),
        ("Exact(12pt)", Some(LineHeight::Exact(Points::new(12.0)))),
        ("Exact(14pt)", Some(LineHeight::Exact(Points::new(14.0)))),
        (
            "AtLeast(12pt)",
            Some(LineHeight::AtLeast(Points::new(12.0))),
        ),
        (
            "AtLeast(14pt)",
            Some(LineHeight::AtLeast(Points::new(14.0))),
        ),
    ] {
        for size in [9.0_f32, 11.0, 12.0, 14.0] {
            let overflow = worst_overflow(lh, size, None);
            assert!(
                overflow <= crate::geometry::LAYOUT_EPSILON_PT,
                "{label} at {size}pt cuts {overflow}pt off a squiggle band",
            );
        }
    }
}

/// The pre-fix figures, pinned as a guard on the *sweep* rather than on the fix.
///
/// If a future change to line metrics or the clip floor moved every combination
/// into the clean region, the sweep above would pass while testing nothing. This
/// asserts the fixture still produces a fragment split with squiggles adjacent to
/// the boundary — the condition the sweep needs in order to mean anything.
#[test]
fn the_sweep_still_exercises_a_boundary() {
    let squiggles = boundary_adjacent_squiggle_count(None, 14.0);
    assert!(
        squiggles > 0,
        "the 14pt default-leading fixture no longer puts squiggles next to a \
         fragment boundary, so the condition sweep asserts nothing",
    );
}
