// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Where** the two paths break, not just how many times — ADR-0017 §5.5.
//!
//! §5.4 established that a paragraph containing a genuinely different run wraps
//! differently on the canvas and DOM paths. A line *count* says they disagree; it
//! cannot say where, and the candidate causes (per-box rounding at a span
//! boundary, a difference in how the two set Parley up across a style change,
//! variable-font instance selection) predict different places.
//!
//! This prints the canvas path's answer at one width: for each line, the text it
//! carries, its advance, and its trailing whitespace. Read it against a shot of
//! the DOM path at the same width — `LB_FIXTURE=… STYLED_WIDTHS=<px>
//! scripts/sitting/run.sh styledlinebreak` leaves one in `target/sitting`.
//!
//! ```text
//! LB_FIXTURE=mixed:family LB_WIDTH=431 \
//!     cargo run -p loki-text --example styled_linebreak_lines
//! ```
//!
//! Advances are in the same units the layout works in — points — so a line that
//! carries the same words on both paths but a different advance is a **metrics**
//! difference, while a line that carries different words is a **break-rule**
//! difference. They are not the same defect and they do not have the same fix.

use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, layout_document,
    resolve::flatten_paragraph_with_base,
};

// The document under test, shared with both halves of the comparison.
#[path = "common/fixture.rs"]
mod fixture;

/// CSS px → points, the ratio Blitz resolves `pt` at.
const PX_TO_PT: f32 = 72.0 / 96.0;

fn main() {
    let (name, doc) = fixture::from_env();
    let width_px: f32 = std::env::var("LB_WIDTH")
        .ok()
        .and_then(|w| w.parse().ok())
        .unwrap_or(431.0);
    let mut fonts = FontResources::new();
    println!(
        "fixture {name} at {width_px} px ({:.2} pt)",
        width_px * PX_TO_PT
    );

    // `preserve_for_editing` is what retains the Parley layout, and the Parley
    // layout is the only thing that knows a line's text range. Read-only mode
    // drops it, which is right for rendering and useless here.
    let options = LayoutOptions {
        preserve_for_editing: true,
        ..LayoutOptions::default()
    };
    let laid = layout_document(
        &mut fonts,
        &doc,
        LayoutMode::Reflow {
            available_width: width_px * PX_TO_PT,
        },
        1.0,
        &options,
    );
    let DocumentLayout::Continuous(continuous) = laid else {
        eprintln!("reflow mode did not produce a continuous layout");
        std::process::exit(1);
    };

    for para in &continuous.paragraphs {
        let Some(text) = paragraph_text(&doc, para.block_index) else {
            continue;
        };
        let Some(parley) = para.layout.parley_layout.as_ref() else {
            eprintln!("block {}: no retained Parley layout", para.block_index);
            continue;
        };
        println!("\nblock {} — {} bytes", para.block_index, text.len());
        let mut end = 0usize;
        for (i, line) in parley.lines().enumerate() {
            let range = line.text_range();
            end = range.end;
            let metrics = line.metrics();
            // `get` rather than indexing: a range that is not a char boundary
            // would panic, and this is a diagnostic — it should say so instead.
            let slice = text.get(range.clone()).unwrap_or("<not a char boundary>");
            println!(
                "  {i:>2} [{:>4}..{:>4}] advance={:>8.3} trailing_ws={:>6.3}  {slice:?}",
                range.start, range.end, metrics.advance, metrics.trailing_whitespace,
            );
        }
        // Which face this paragraph actually got, and whether the canvas path
        // is *synthesising* the weight rather than using a bold face. §5.6's
        // residual is a run set in Tinos Bold coming out 0.13 % wider here than
        // in the DOM, and synthetic emboldening is one of the two ways that can
        // happen — the other being a different face — so both are printed.
        for item in &para.layout.items {
            if let loki_layout::items::PositionedItem::GlyphRun(run) = item {
                println!(
                    "     face: {} bytes idx={} size={:.3} synthesis={:?}",
                    run.font_data.len(),
                    run.font_index,
                    run.font_size,
                    run.synthesis,
                );
                break;
            }
        }
        // The line ranges index the text Parley was given, which is the
        // flattened text only when nothing was cleaned out of it (fields, notes,
        // hidden revisions). Checked rather than assumed: a silent offset would
        // make every printed line one word wrong, which reads as a rendering
        // difference.
        if end != text.len() {
            println!(
                "  WARNING: lines cover {end} bytes of {} — the text was cleaned, \
                 so these slices are offset",
                text.len()
            );
        }
    }
}

/// The flattened text of the top-level paragraph at `block_index`.
///
/// Flattened by `loki_layout`'s own function — the same one the layout call used
/// — so the byte ranges Parley reports index this string and not a different
/// concatenation of the same runs.
fn paragraph_text(doc: &loki_doc_model::Document, block_index: usize) -> Option<String> {
    use loki_doc_model::content::block::Block;
    let block = doc
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .nth(block_index)?;
    let owned;
    let para = match block {
        Block::StyledPara(p) => p,
        Block::Para(items) | Block::Plain(items) => {
            owned = loki_layout::flow::synthesize_plain_para(items);
            &owned
        }
        Block::Heading(level, attr, items) => {
            owned = loki_layout::flow::synthesize_heading_para(*level, attr, items);
            &owned
        }
        _ => return None,
    };
    let mut notes = 0u32;
    let (text, ..) = flatten_paragraph_with_base(
        para,
        &doc.styles,
        &mut notes,
        None,
        loki_layout::RevisionDisplay::default(),
    );
    Some(text)
}
