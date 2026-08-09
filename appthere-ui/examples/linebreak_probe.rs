// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The DOM half of **ADR-0017's line-break comparison** — the test that ADR
//! sequences first, because it can end the decision.
//!
//! # The question
//!
//! ADR-0017 moves the reflow view from canvas to DOM. The canvas path shapes
//! with Parley against `loki-layout`'s own resolution of the document's
//! character properties; the DOM path shapes with Parley through Stylo's CSS
//! cascade, from styles we emit. If a paragraph sets differently through the
//! two, every document reflows differently than it does today — a fidelity
//! regression that would sink the ADR.
//!
//! # The instrument
//!
//! One paragraph at several column widths, each on a solid background with an
//! **explicit `line-height`**, so the line count is readable from a screenshot
//! as `block height / line height` — no text recognition needed, and no
//! dependence on either path's natural leading (which differs, and is not what
//! this asks about).
//!
//! Pair it with a sweep of `loki_layout::layout_document` in
//! `LayoutMode::Reflow` over the same text, family and size, counting distinct
//! glyph-run baselines. Widths here are CSS px; the layout side takes points
//! (× 72/96).
//!
//! **Set [`WIDTHS`] to the layout side's *transition* widths, ±2 px.** Coarse
//! widths agreeing proves little — the break is unambiguous there. Where the
//! count changes is where the decision is marginal, and marginal is where two
//! shapers disagree first.
//!
//! # Result, 2026-08-03
//!
//! All seven transitions matched, each pinned to a 2 px window: 10→9 at 182,
//! 9→8 at 192, 8→7 at 218, 7→6 at 272, 6→5 at 306, 5→4 at 370, 4→3 at 520.
//! See ADR-0017 §3.2.

use dioxus::prelude::*;

const TEXT: &str = "The quick brown fox jumps over the lazy dog while the rest of \
the type sets itself into an ordinary line of running text that has to break \
somewhere sensible and keep breaking as the column narrows around it.";

/// 12 pt at 96 dpi. The layout side is given 12.0 pt.
const FONT_PX: f32 = 16.0;
/// Explicit, so `height / LINE_PX` is the line count exactly.
const LINE_PX: f32 = 24.0;

const WIDTHS: [f32; 14] = [
    190.0, 192.0, 216.0, 218.0, 270.0, 272.0, 304.0, 306.0, 368.0, 370.0, 518.0, 520.0, 180.0,
    182.0,
];

#[component]
fn Probe() -> Element {
    rsx! {
        div {
            style: "width: 100vw; height: 100vh; background: rgb(0,0,0); \
                    display: flex; flex-direction: column; gap: 6px; margin: 0;",
            for (i, w) in WIDTHS.iter().copied().enumerate() {
                div {
                    key: "{i}",
                    style: format!(
                        "width: {w}px; font-family: 'Liberation Sans'; \
                         font-size: {FONT_PX}px; line-height: {LINE_PX}px; \
                         background: rgb(255, 255, 255); color: rgb(0,0,0); \
                         flex-shrink: 0;"
                    ),
                    "{TEXT}"
                }
            }
        }
    }
}

fn main() {
    dioxus::native::launch_cfg(
        Probe,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new().with_window_attributes(
                dioxus::native::WindowAttributes::default()
                    .with_title("linebreak-probe")
                    .with_inner_size(dioxus::native::LogicalSize::new(700.0, 2400.0)),
            ),
        )],
    );
}
