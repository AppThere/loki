// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The **layout half** of ADR-0017's styled-document line-break comparison
//! (§5.3 step 3).
//!
//! # Why a second comparison
//!
//! §3.2's comparison ran one paragraph, one face, one size, no named styles and
//! no font substitution. The DOM reflow view has since grown three things that
//! change what is being compared: catalog resolution (§5.1), family substitution
//! through `resolve_font_name` (§5.1a), and heading/bare-paragraph synthesis. A
//! substituted face has different metrics, so a comparison run on a family both
//! paths could resolve never exercised the path a real document takes.
//!
//! # The instrument
//!
//! Lays the **screenplay template** out through `loki_layout::layout_document`
//! in `LayoutMode::Reflow` across a range of column widths, counting distinct
//! glyph-run baselines — the same count the canvas path would paint. Prints the
//! per-width totals and, more usefully, the **transition** widths where the
//! count changes.
//!
//! Transitions are the discriminating points. At a coarse width the break is
//! unambiguous and agreement proves little; where the count changes the decision
//! is marginal, and marginal is where two shapers diverge first.
//!
//! Feed the printed transitions to `styled_linebreak_probe`, which renders the
//! same document through the DOM path at each transition ± 1 px.
//!
//! # Units
//!
//! Widths are reported in **CSS px** to match §3.2's table, and converted to
//! points (× 72/96) for the layout call — points are what `LayoutMode::Reflow`
//! takes and what the DOM column's `max-width` is authored in.
//!
//! ```text
//! cargo run -p loki-text --example styled_linebreak_sweep
//! ```

use loki_layout::items::PositionedItem;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};

// The document under test, shared with the DOM half rather than restated here —
// see the module docs there on why two copies would be worse than none.
#[path = "common/fixture.rs"]
mod fixture;

/// Inclusive px range and step for the sweep.
///
/// The lower bound is where the screenplay's deepest indent (the character cue,
/// 158 pt from the text margin) still leaves a usable column; below it the
/// layout is degenerate and the comparison is about clamping rather than
/// line-breaking.
const MIN_PX: i32 = 260;
const MAX_PX: i32 = 800;
const STEP_PX: i32 = 1;

/// CSS px → points, the ratio Blitz resolves `pt` at.
const PX_TO_PT: f32 = 72.0 / 96.0;

/// Distinct glyph-run baselines in a reflow layout at `width_pt`.
///
/// Two runs on one line share an origin; two paragraphs are stacked, so their
/// lines cannot collide. The count is therefore the document's line count.
fn line_count(fonts: &mut FontResources, doc: &loki_doc_model::Document, width_pt: f32) -> usize {
    let layout = layout_document(
        fonts,
        doc,
        LayoutMode::Reflow {
            available_width: width_pt,
        },
        1.0,
        &LayoutOptions::default(),
    );
    let mut ys: Vec<f32> = layout
        .all_items()
        .filter_map(|item| match item {
            PositionedItem::GlyphRun(run) => Some(run.origin.y),
            _ => None,
        })
        .collect();
    ys.sort_by(f32::total_cmp);
    // A tolerance rather than exact equality: two runs on one line are placed
    // from the same baseline but reach it through different accumulations.
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    ys.len()
}

fn main() {
    let (name, doc) = fixture::from_env();
    let mut fonts = FontResources::new();
    println!("fixture: {name}");

    // The substitutions the DOM path mirrors. Printed rather than assumed: on a
    // host that *has* these families the run does not exercise substitution at
    // all, and the result must not be read as though it did.
    for requested in ["Arial", "Courier New", "Times New Roman"] {
        let resolved = fonts.resolve_font_name(requested);
        println!("family {requested:?} -> {resolved:?}");
        if resolved == requested {
            println!("  NOTE: no substitution for {requested:?} on this host.");
        }
    }

    let mut prev: Option<(i32, usize)> = None;
    let mut transitions: Vec<(i32, usize, usize)> = Vec::new();
    let mut px = MIN_PX;
    while px <= MAX_PX {
        let n = line_count(&mut fonts, &doc, px as f32 * PX_TO_PT);
        println!("{px} px\t{:.2} pt\t{n} lines", px as f32 * PX_TO_PT);
        if let Some((_, pn)) = prev
            && pn != n
        {
            transitions.push((px, pn, n));
        }
        prev = Some((px, n));
        px += STEP_PX;
    }

    println!("\ntransitions (px at which the count changes):");
    for (px, from, to) in &transitions {
        println!("  {from} -> {to} at {px} px  (probe {} and {px})", px - 1);
    }
    println!("{} transitions", transitions.len());

    // Machine-readable, for `scripts/sitting/run.sh styledlinebreak` to hand
    // straight to the DOM half. Printed rather than transcribed: a width list
    // typed into the scenario by hand is a second copy of this sweep's answer,
    // and the first thing that copy loses is which fixture it came from.
    let widths: Vec<String> = transitions
        .iter()
        .flat_map(|(px, ..)| [(px - 1).to_string(), px.to_string()])
        .collect();
    println!("probe-widths {}", widths.join(","));
    // Calibrate on the **widest** column: the fewest lines, so the constant is
    // read where the least is stacked on top of it.
    if let Some((px, _, to)) = transitions.last() {
        println!("calibrate {px}:{to}");
    }
}
