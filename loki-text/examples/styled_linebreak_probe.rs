// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The **DOM half** of ADR-0017's styled-document line-break comparison
//! (§5.3 step 3). Pair it with `styled_linebreak_sweep`, which is the layout
//! half and prints the widths to run here.
//!
//! # It renders through the real view, not a copy
//!
//! The tree comes from `dom_reflow::document_view` and the family map from
//! `dom_reflow::resolve_families` — the same two functions the editor calls.
//! A probe with its own CSS emitter would agree with the canvas path exactly as
//! far as the copy did, and the copy is what drifts.
//!
//! # How the line count is read
//!
//! Each column is one full render of the screenplay template at one width, laid
//! out in a row on a **blue** page. A column paints its own white background, so
//! its height is a white band whose bottom edge is measurable from a screenshot
//! with no text recognition — and `line-height` is pinned on the root, so a band
//! is `constant + lines × 24 px`. Two widths that differ by one line differ by
//! exactly 24 px of band.
//!
//! The constant (padding plus the paragraph margins, neither of which depends on
//! width) is calibrated once against the layout half's count at one width; the
//! same constant then has to reproduce every other width's count. That is the
//! comparison.
//!
//! # Running it
//!
//! ```text
//! STYLED_WIDTHS=268,269,278,279 cargo run -p loki-text \
//!     --example styled_linebreak_probe
//! ```
//!
//! Widths are CSS px and are the *content* column width — the same number the
//! sweep prints, and `× 72/96` the points it hands `LayoutMode::Reflow`.
//!
//! `scripts/sitting/linebreak_bands.py` reads the screenshot and this program's
//! stdout. It finds the bands **in the image** rather than from an x offset
//! printed here: the page carries the UA's 8 px body margin, which shifts every
//! band, and a first attempt that trusted the arithmetic sampled the gaps
//! between columns and reported eight-pixel bands.

use dioxus::prelude::*;
use loki_layout::{FontResources, SharedFontResources};
use loki_text::routes::editor::dom_reflow;

/// Column padding, from the view's own `padding: 24pt 18pt` — 18 pt each side.
const PAD_PX: f32 = 48.0;
/// Gap between columns, so two bands never touch.
const GAP_PX: f32 = 8.0;
/// Pinned so a band's height is a whole number of lines.
const LINE_PX: f32 = 24.0;

/// The widths to render, from `STYLED_WIDTHS` or the sweep's nine transitions.
fn widths() -> Vec<f32> {
    let from_env: Vec<f32> = std::env::var("STYLED_WIDTHS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|w| w.trim().parse::<f32>().ok())
        .collect();
    if from_env.is_empty() {
        // The nine transitions the layout-side sweep reported on 2026-08-04,
        // each bracketed by the px below it. An unset *and* an empty variable
        // both land here: the scenario passes `STYLED_WIDTHS=""` when the
        // caller did not set one, and an empty list would render no columns at
        // all — a blank shot that the measuring script reads as a mismatch
        // rather than as "the probe was misconfigured".
        return vec![
            268.0, 269.0, 278.0, 279.0, 297.0, 298.0, 345.0, 346.0, 355.0, 356.0, 403.0, 404.0,
            422.0, 423.0, 528.0, 529.0, 585.0, 586.0,
        ];
    }
    from_env
}

#[component]
fn Probe() -> Element {
    let Some(doc) = loki_templates::document("screenplay") else {
        return rsx! { div { "the screenplay template did not load" } };
    };
    let fonts = SharedFontResources::new_ready(FontResources::new());
    let families = dom_reflow::resolve_families(&fonts, &doc);
    let ws = widths();

    rsx! {
        div {
            // Blue page, so a column's white band has an edge to measure
            // against. `align-items: flex-start` keeps each band's height its
            // own content height rather than the row's.
            style: format!(
                "width: 100vw; height: 100vh; background: rgb(0, 0, 255); \
                 display: flex; flex-direction: row; align-items: flex-start; \
                 gap: {GAP_PX}px; line-height: {LINE_PX}px;"
            ),
            for (i, w) in ws.iter().copied().enumerate() {
                div {
                    key: "{i}",
                    // Exactly the column's border-box width, so the band's x
                    // range is the wrapper's and the content width is `w`.
                    style: format!("width: {}px; flex-shrink: 0;", w + PAD_PX),
                    { dom_reflow::document_view(&doc, &families, w * 0.75) }
                }
            }
        }
    }
}

fn main() {
    let ws = widths();
    let total: f32 = ws.iter().map(|w| w + PAD_PX + GAP_PX).sum::<f32>() - GAP_PX;
    // The widths in row order, which is all the measuring script needs to label
    // the bands it finds; it derives their positions from the image.
    for w in &ws {
        println!("band {w} expect_width={}", w + PAD_PX);
    }
    println!("total {total}");
    // What the run actually resolved. Printed rather than assumed: on a host
    // that has Courier New this run does not exercise substitution at all, and
    // the result must not be read as though it did.
    if let Some(doc) = loki_templates::document("screenplay") {
        let fonts = SharedFontResources::new_ready(FontResources::new());
        for (requested, resolved) in dom_reflow::resolve_families(&fonts, &doc) {
            println!("family {requested:?} -> {resolved:?}");
        }
    }

    dioxus::native::launch_cfg(
        Probe,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new()
                // **The same registration `main.rs` does.** `resolve_font_name`
                // answers with a family from `loki-layout`'s collection, and
                // Blitz has its own — a name resolved in one is not resolvable
                // in the other unless the bytes are registered in both. Without
                // this line the probe renders the screenplay in Blitz's default
                // sans while the canvas path sets it in the bundled Cousine,
                // and the comparison measures the probe's launch config rather
                // than the view.
                .with_fonts(loki_fonts::ui_font_blobs())
                .with_window_attributes(
                    dioxus::native::WindowAttributes::default()
                        .with_title("styled-linebreak-probe")
                        .with_inner_size(dioxus::native::LogicalSize::new(total + 4.0, 900.0)),
                ),
        )],
    );
}
