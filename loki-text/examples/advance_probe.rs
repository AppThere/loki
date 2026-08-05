// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The DOM half of ADR-0017 §5.6 — **how wide is this run, really**.
//!
//! §5.5 left a residual of order 0.1 % of a line's advance: enough to flip a
//! break that was already marginal, too small to see in a line *count*. This
//! measures the quantity directly.
//!
//! # The instrument
//!
//! One row per resolved style, all carrying the same string
//! ([`fixture::ADVANCE_TEXT`]), each in a `<span>` that emits **exactly the CSS
//! the reflow view emits** (`dom_reflow::style::span_css`).
//!
//! The ruler is the row **box**, not the glyphs: each row is a white block in a
//! `align-items: flex-start` column, so it is sized `fit-content`, and
//! `white-space: pre` keeps it to one line. Its width is therefore the run's
//! advance in CSS px — no glyph bounds, no side bearings, no text recognition.
//!
//! (A background on the `<span>` itself would be the more direct ruler and is
//! what this first tried; Blitz does not paint backgrounds on non-replaced
//! inline boxes, so the rectangle never appeared. The row box measures the same
//! quantity, up to Taffy's rounding of a box to whole pixels — which is why the
//! scale below matters.)
//!
//! Pair it with `styled_linebreak_lines`, which prints the canvas path's advance
//! in points for the same document; `scripts/sitting/advance_bands.py` measures
//! the rectangles and does the comparison.
//!
//! # Why a scale
//!
//! `LB_FIXTURE=advances:<scale>` multiplies every font size and the tracked
//! case's letter-spacing. A **rounding** difference is a fixed number of pixels
//! and stays put as the type grows; a **metrics** difference is a fraction and
//! grows with it. Measuring at two scales is what separates them, and neither is
//! visible at 12 pt where the whole effect is a quarter of a pixel.
//!
//! ```text
//! LB_FIXTURE=advances:8 scripts/sitting/run.sh advances
//! ```

use dioxus::prelude::*;
use loki_layout::{FontResources, SharedFontResources};
use loki_text::routes::editor::dom_reflow;

// The document under test, shared with the canvas half.
#[path = "common/fixture.rs"]
mod fixture;

/// Vertical gap between rows, so two red rectangles are never one band.
const GAP_PX: f32 = 12.0;

/// The scale from `LB_FIXTURE=advances:<scale>`, for sizing the window.
fn scale() -> f32 {
    std::env::var("LB_FIXTURE")
        .ok()
        .and_then(|s| s.split_once(':').and_then(|(_, v)| v.parse().ok()))
        .unwrap_or(1.0)
}

#[component]
fn Probe() -> Element {
    let (_, doc) = fixture::from_env();
    let fonts = SharedFontResources::new_ready(FontResources::new());
    let families = dom_reflow::resolve_families(&fonts, &doc);
    let rows = rows(&doc, &families);

    rsx! {
        div {
            style: format!(
                "width: 100vw; height: 100vh; background: rgb(0, 0, 255); \
                 display: flex; flex-direction: column; align-items: flex-start; \
                 gap: {GAP_PX}px;"
            ),
            for (i, (css, features, text)) in rows.into_iter().enumerate() {
                div {
                    key: "{i}",
                    style: "background: rgb(255, 255, 255); white-space: pre;",
                    // The view's own CSS and nothing else: anything added here
                    // would make this a measurement of a different run than the
                    // one the view renders.
                    span {
                        style: "{css}",
                        "data-font-features": "{features}",
                        "{text}"
                    }
                }
            }
        }
    }
}

/// The emitted CSS for each case's single run, in document order.
///
/// Resolved through `loki_layout`'s flattener — the same call
/// `content::styled_para_el` makes — so a row is the run the view would emit and
/// not a second reading of the same properties.
fn rows(
    doc: &loki_doc_model::document::Document,
    families: &dom_reflow::content::FamilyMap,
) -> Vec<(String, &'static str, String)> {
    use loki_doc_model::content::block::Block;
    let mut out = Vec::new();
    for block in doc.sections.iter().flat_map(|s| s.blocks.iter()) {
        let Block::StyledPara(para) = block else {
            continue;
        };
        let mut notes = 0u32;
        let (text, spans, _, _) = loki_layout::resolve::flatten_paragraph_with_base(
            para,
            &doc.styles,
            &mut notes,
            None,
            loki_layout::RevisionDisplay::default(),
        );
        // One run per paragraph by construction; a second would mean the fixture
        // changed under this probe, and a silently-dropped one would measure the
        // wrong string.
        if spans.len() != 1 {
            eprintln!("expected one run per advance case, got {}", spans.len());
        }
        if let Some(span) = spans.first() {
            // The text comes from the flattener too, so the probe cannot render
            // a different string from the one the canvas half measured.
            out.push((
                dom_reflow::style::span_css(span, families),
                dom_reflow::style::span_font_features(span),
                text.clone(),
            ));
        }
    }
    out
}

fn main() {
    let s = scale();
    // Sized from the scale rather than fixed: the widest case at scale 8 is
    // ~3.7 k px, and a row clipped by the window measures as a shorter advance.
    let width = 500.0 * s + 400.0;
    let height = fixture::advance_cases().len() as f32 * (20.0 * s + GAP_PX) + 60.0;
    for (name, ..) in fixture::advance_cases() {
        println!("case {name}");
    }
    println!("scale {s} window {width}x{height}");
    let (_, doc) = fixture::from_env();
    let fonts = SharedFontResources::new_ready(FontResources::new());
    for (requested, resolved) in dom_reflow::resolve_families(&fonts, &doc) {
        println!("family {requested:?} -> {resolved:?}");
    }

    dioxus::native::launch_cfg(
        Probe,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new()
                // The same registration `main.rs` does — without it a substituted
                // family is a name Blitz cannot resolve, and every row would be
                // measuring the fallback face (ADR-0017 §5.3).
                .with_fonts(loki_fonts::ui_font_blobs())
                .with_window_attributes(
                    dioxus::native::WindowAttributes::default()
                        .with_title("advance-probe")
                        .with_inner_size(dioxus::native::LogicalSize::new(width, height)),
                ),
        )],
    );
}
