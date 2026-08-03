// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Probe P1 (Spec 08 T7.0)** — a scratch nested scroll container.
//!
//! T7.3 wants an oversized element (a wide table, a wide image) to expand into
//! **its own horizontal scroll container** inside the vertically scrolling
//! document, so the document itself never scrolls sideways. That only works if
//! a nested scroll container *routes* input correctly: consumes a gesture inside
//! its own bounds, and bubbles what it cannot use to its scrolling ancestor.
//!
//! # Why this is an input test, not a layout test
//!
//! `blitz-dom` models the geometry already — `scroll_node_by_collect_inner`
//! clamps each axis to the node's scroll range and recurses into the parent with
//! the remainder. Read on its own it is plainly correct. The plausible failure
//! is one layer up, in `blitz-shell`'s `MouseWheel` arm, which chooses *which
//! node* that walk starts from:
//!
//! ```text
//! get_hover_node_id().or_else(get_focussed_node_id())  →  scroll_node_within_collect(node_id, …)
//! ```
//!
//! Both parts of that can render right and route wrong. If the hover node is
//! the outer container rather than the inner one under the cursor, the inner
//! never gets the gesture. If the hover node is stale — it updates only on
//! cursor-move — the fallback sends the gesture to the *focused* node, which is
//! somewhere else entirely. Neither shows up in a layout assertion.
//!
//! # What the scene is built to discriminate
//!
//! Three bands, so a screenshot can say *which* container moved rather than
//! only that something did:
//!
//! - The **outer** container scrolls vertically and holds numbered rows down
//!   its left edge, visible above and below the inner box.
//! - The **inner** container sits in the middle, scrolls vertically, and holds
//!   its own differently-coloured numbered rows.
//! - A **static** band at the very top scrolls with nothing, so a run in which
//!   the whole window moved is distinguishable from one in which a container
//!   did.
//!
//! Each row is a solid colour block wide enough to survive downscaling, because
//! the reading is a pixel diff of cropped bands and not text recognition.
//!
//! # Two scenes
//!
//! `PROBE_SCENE=vertical` (default) nests a vertically scrolling box inside a
//! vertically scrolling one: the simplest case, where both containers want the
//! same axis and the question is purely who gets it first.
//!
//! `PROBE_SCENE=horizontal` is **the configuration T7.3 actually ships**: a
//! horizontally scrolling box (`overflow-x: auto; overflow-y: hidden`) inside
//! the vertically scrolling document. It asks a different question, and the
//! more dangerous one — a vertical gesture over the wide table must *not* be
//! swallowed by a container that cannot use it, or the document stops scrolling
//! wherever the user's pointer happens to rest.
//!
//! Run under the sitting harness: `scripts/sitting/run.sh nestedscroll`.

use dioxus::prelude::*;

/// Height of one row, in logical pixels. Large enough that a single wheel notch
/// (20 px in `blitz-shell`'s line-delta conversion) cannot move a full row, so
/// the number of notches sent maps to a predictable offset.
const ROW_H: u32 = 40;

/// Rows in each container. The outer needs enough to overflow the window; the
/// inner needs enough to overflow its own box **and** few enough that a bounded
/// number of notches reaches its end — the third reading depends on getting the
/// inner to its limit.
const OUTER_ROWS: u32 = 40;

/// Outer rows placed *before* the inner box. Few enough that the inner box is
/// on screen at rest: a probe whose subject has to be scrolled into view would
/// need the very gesture it is measuring in order to set itself up.
const ROWS_BEFORE_INNER: u32 = 4;
const INNER_ROWS: u32 = 8;

/// The inner container's visible height — two rows, so six of its eight rows are
/// out of view and it takes a known number of notches to exhaust.
const INNER_VIEW_H: u32 = ROW_H * 2;

fn row(i: u32, hue: u32, tag: &str) -> Element {
    // Distinct luminance per row so a crop of any single row identifies which
    // row is at that position — the measurement is "did this band change", and
    // a uniform fill would report no change even when it scrolled by a row.
    let shade = 40 + (i * 23) % 180;
    rsx! {
        div {
            key: "{tag}-{i}",
            style: format!(
                "height: {ROW_H}px; background: rgb({r}, {g}, {b}); \
                 color: white; font-size: 20px; padding-left: 8px;",
                r = if hue == 0 { shade } else { 20 },
                g = if hue == 1 { shade } else { 20 },
                b = if hue == 2 { shade } else { 20 },
            ),
            "{tag}{i}"
        }
    }
}

/// A row of wide blocks — content that overflows horizontally, for the
/// `horizontal` scene.
fn wide_strip() -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; width: 3000px;",
            for i in 0..12u32 {
                div {
                    key: "W-{i}",
                    style: format!(
                        "width: 250px; height: {h}px; background: rgb(20, {g}, 20); \
                         color: white; font-size: 20px; flex-shrink: 0;",
                        h = INNER_VIEW_H,
                        g = 40 + (i * 17) % 180,
                    ),
                    "W{i}"
                }
            }
        }
    }
}

#[component]
fn Probe() -> Element {
    // Read once at mount: the scene is a launch-time choice, not a live setting.
    let horizontal = std::env::var("PROBE_SCENE").as_deref() == Ok("horizontal");
    rsx! {
        div {
            style: "width: 100vw; height: 100vh; display: flex; flex-direction: column; \
                    background: rgb(8, 8, 8); margin: 0;",

            // A band that must never move. Without it a run where the whole
            // window scrolled reads identically to one where the outer
            // container did — the control that separates "a container
            // consumed this" from "something moved".
            div {
                id: "static-band",
                style: "height: 60px; background: rgb(200, 200, 0); color: black; \
                        font-size: 24px; flex-shrink: 0;",
                "STATIC"
            }

            // The outer scroll container.
            div {
                id: "outer",
                style: "flex: 1; overflow-y: auto; overflow-x: hidden; background: rgb(0, 0, 40);",

                for i in 0..ROWS_BEFORE_INNER {
                    { row(i, 0, "O") }
                }

                // The inner scroll container, sitting inside the outer's
                // content so a gesture over it has a genuine ancestor to
                // bubble to.
                div {
                    id: "inner",
                    style: format!(
                        "height: {INNER_VIEW_H}px; overflow-y: {oy}; overflow-x: {ox}; \
                         background: rgb(0, 40, 0); border: 4px solid rgb(0, 255, 255); \
                         margin: 8px 40px;",
                        oy = if horizontal { "hidden" } else { "auto" },
                        ox = if horizontal { "auto" } else { "hidden" },
                    ),
                    if horizontal {
                        { wide_strip() }
                    } else {
                        for i in 0..INNER_ROWS {
                            { row(i, 1, "I") }
                        }
                    }
                }

                for i in ROWS_BEFORE_INNER..OUTER_ROWS {
                    { row(i, 0, "O") }
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
                    .with_title("nested-scroll-probe")
                    .with_inner_size(dioxus::native::LogicalSize::new(900.0, 700.0)),
            ),
        )],
    );
}
