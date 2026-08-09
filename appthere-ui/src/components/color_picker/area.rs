// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtColorArea` — the saturation/value square and the hue strip beside it
//! (Spec 08 T5.2, I-09).
//!
//! # Gradients, not a grid of cells
//!
//! The square is a hue-coloured background under two CSS gradients: white to
//! transparent across, and transparent to black down. `blitz-paint` implements
//! linear gradients, so this is one element rather than the hundreds of small
//! divs the alternative needs — and hundreds of divs would be laid out and
//! painted on every drag frame.
//!
//! # Coordinates come from the measured rect, not from `element_coordinates`
//!
//! `editor_pointer` already records that `element_coordinates` is dependable
//! only on the per-tile path, and uses `client_coordinates` against a rect it
//! measured. This follows that: the square reports its rect on mount, and a
//! pointer position is that rect subtracted from the client coordinate. Same
//! mechanism as the zoom control's anchor.
//!
//! # Touch target
//!
//! The square and strip are drag surfaces rather than buttons, so the 44 px
//! minimum applies to the *strip's width* (which is the narrow axis a finger has
//! to land on) — it is [`TOUCH_MIN`] wide. The square is far larger than 44 px
//! in both axes.

use dioxus::prelude::*;

use super::area_geom::{
    hue_from_position, hue_strip_gradient, position_from_hue, position_from_sv, sv_from_position,
};
use super::convert::{hsv_to_rgb, rgb_to_hex};
use crate::tokens::colors::COLOR_BORDER_CHROME;
use crate::tokens::spacing::{RADIUS_SM, SPACE_2, TOUCH_MIN};

/// Height of the square and the strip, in logical pixels.
const AREA_HEIGHT_PX: f32 = 160.0;
/// Width of the square.
const AREA_WIDTH_PX: f32 = 200.0;
/// Diameter of the draggable handle.
const HANDLE_PX: f32 = 14.0;

/// A measured rectangle, in client coordinates.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Measured {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// Props for [`AtColorArea`].
#[derive(Props, Clone, PartialEq)]
pub struct AtColorAreaProps {
    /// Current hue, 0–360.
    pub hue: f32,
    /// Current saturation, 0–100.
    pub saturation: f32,
    /// Current value, 0–100.
    pub value: f32,
    /// Accessible name of the saturation/value square.
    pub area_label: String,
    /// Accessible name of the hue strip.
    pub hue_label: String,
    /// A new hue/saturation/value, as the reader drags.
    pub on_change: EventHandler<(f32, f32, f32)>,
}

/// The saturation/value square and hue strip.
#[component]
pub fn AtColorArea(props: AtColorAreaProps) -> Element {
    let square = use_signal(Measured::default);
    let strip = use_signal(Measured::default);
    let mut dragging_square = use_signal(|| false);
    let mut dragging_strip = use_signal(|| false);

    let (hue, sat, val) = (props.hue, props.saturation, props.value);
    let (hr, hg, hb) = hsv_to_rgb(hue, 100.0, 100.0);
    let pure_hue = rgb_to_hex(hr, hg, hb);
    let (hx, hy) = position_from_sv(sat, val, AREA_WIDTH_PX, AREA_HEIGHT_PX);
    let strip_y = position_from_hue(hue, AREA_HEIGHT_PX);

    let apply_square = move |x: f32, y: f32| {
        let m = *square.peek();
        if m.w > 0.0 {
            let (s, v) = sv_from_position(x - m.x, y - m.y, m.w, m.h);
            props.on_change.call((hue, s, v));
        }
    };
    let apply_strip = move |y: f32| {
        let m = *strip.peek();
        if m.h > 0.0 {
            props
                .on_change
                .call((hue_from_position(y - m.y, m.h), sat, val));
        }
    };

    rsx! {
        div {
            style: format!("display: flex; gap: {SPACE_2}px; align-items: flex-start;"),

            // ── Saturation / value square ────────────────────────────────────
            div {
                role: "slider",
                aria_label: props.area_label.clone(),
                onmounted: move |e: MountedEvent| { spawn(measure(e, square)); },
                onmousedown: move |e: MouseEvent| {
                    dragging_square.set(true);
                    let c = e.client_coordinates();
                    apply_square(c.x as f32, c.y as f32);
                },
                onmousemove: move |e: MouseEvent| {
                    if dragging_square() {
                        let c = e.client_coordinates();
                        apply_square(c.x as f32, c.y as f32);
                    }
                },
                onmouseup: move |_| dragging_square.set(false),
                // A drag that leaves the square keeps selecting until the button
                // is released — the geometry clamps, so the colour pins to the
                // nearest edge rather than freezing where the pointer left.
                onmouseleave: move |_| dragging_square.set(false),
                style: format!(
                    "position: relative; width: {w}px; height: {h}px; \
                     border-radius: {r}px; border: 1px solid {bd}; cursor: crosshair; \
                     background: linear-gradient(to top, #000000, rgba(0,0,0,0)), \
                     linear-gradient(to right, #FFFFFF, rgba(255,255,255,0)), {hue};",
                    w = AREA_WIDTH_PX, h = AREA_HEIGHT_PX, r = RADIUS_SM,
                    bd = COLOR_BORDER_CHROME, hue = pure_hue,
                ),
                div { style: handle_style(hx, hy) }
            }

            // ── Hue strip ────────────────────────────────────────────────────
            div {
                role: "slider",
                aria_label: props.hue_label.clone(),
                onmounted: move |e: MountedEvent| { spawn(measure(e, strip)); },
                onmousedown: move |e: MouseEvent| {
                    dragging_strip.set(true);
                    apply_strip(e.client_coordinates().y as f32);
                },
                onmousemove: move |e: MouseEvent| {
                    if dragging_strip() {
                        apply_strip(e.client_coordinates().y as f32);
                    }
                },
                onmouseup: move |_| dragging_strip.set(false),
                onmouseleave: move |_| dragging_strip.set(false),
                style: format!(
                    "position: relative; width: {w}px; height: {h}px; \
                     border-radius: {r}px; border: 1px solid {bd}; cursor: ns-resize; \
                     background: {g};",
                    w = TOUCH_MIN, h = AREA_HEIGHT_PX, r = RADIUS_SM,
                    bd = COLOR_BORDER_CHROME, g = hue_strip_gradient(),
                ),
                div { style: handle_style(TOUCH_MIN / 2.0, strip_y) }
            }
        }
    }
}

/// Records an element's client rect once it has mounted.
async fn measure(e: MountedEvent, mut into: Signal<Measured>) {
    if let Ok(r) = e.get_client_rect().await {
        into.set(Measured {
            x: r.origin.x as f32,
            y: r.origin.y as f32,
            w: r.size.width as f32,
            h: r.size.height as f32,
        });
    }
}

/// The draggable handle: a ring, so the colour underneath stays visible.
///
/// White with a dark outline rather than a single colour, because a handle in
/// one colour disappears against part of its own square — which is the half of
/// the square the reader is most likely to be aiming at when it happens.
fn handle_style(x: f32, y: f32) -> String {
    format!(
        "position: absolute; left: {left}px; top: {top}px; \
         width: {d}px; height: {d}px; border-radius: {d}px; \
         border: 2px solid #FFFFFF; outline: 1px solid rgba(0,0,0,0.6); \
         pointer-events: none;",
        left = x - HANDLE_PX / 2.0,
        top = y - HANDLE_PX / 2.0,
        d = HANDLE_PX,
    )
}
