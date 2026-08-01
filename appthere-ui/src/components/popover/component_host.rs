// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtPopoverHost`] — the root-mounted renderer (Spec 08 T4.1).
//!
//! Split from `component.rs` at the 300-line ceiling (r75), when
//! [`OverlayKind`] gave the host its first branch. The request type and the
//! context stay next door; this is the one component.

use std::rc::Rc;

use dioxus::prelude::*;

use super::{AtPopoverContext, OverlayKind};
use crate::components::popover::geometry::Placement;

/// Renders the open popover at the app root.
///
/// # Mount after `AtBackdropHost`
///
/// [`super::host::RootLayer`] states why: `z-index` cannot arbitrate between two
/// children of the positioned root, so DOM order does, and a backdrop painting
/// over the popup makes it visible and unclickable.
///
/// # Touch target
///
/// The host imposes no size of its own; a 44 × 44 px minimum (WCAG 2.5.8) is the
/// responsibility of the interactive rows inside `content`, which the host does
/// not construct.
#[component]
pub fn AtPopoverHost() -> Element {
    let ctx = use_context::<AtPopoverContext>();
    let Some(request) = ctx.open.read().clone() else {
        return rsx! {};
    };
    // Read, never computed. See `AtPopoverContext::resolved`: deriving placement
    // here would take it off the one path the reposition counter watches.
    let Some(Placement { rect, .. }) = *ctx.resolved.read() else {
        return rsx! {};
    };
    let dismiss = Rc::clone(&request.on_dismiss);
    let outside_move = request.on_outside_move.clone();
    let dismissible = request.kind == OverlayKind::Dismissible;
    rsx! {
        // Backdrop first: DOM order is what orders two children of the same
        // positioned root, since `z-index` cannot arbitrate between them.
        //
        // Only for a dismissible overlay. A tooltip's backdrop would be a
        // window-sized click-catcher for something no click dismisses.
        if dismissible {
        div {
            style: format!(
                "position: absolute; top: 0; left: 0; width: 100%; height: 100%; \
                 z-index: {z};",
                z = crate::components::overlay::BACKDROP_Z_INDEX,
            ),
            onclick: move |_| dismiss(),
            onmousemove: move |_| {
                if let Some(f) = outside_move.as_ref() {
                    f();
                }
            },
        }
        }
        div {
            style: format!(
                "position: absolute; left: {left}px; top: {top}px; \
                 width: {width}px; max-height: {height}px; overflow-y: auto; \
                 z-index: {z};",
                left = rect.x,
                top = rect.y,
                width = rect.width,
                // `max-height` with `overflow-y: auto` rather than a fixed
                // height, so a clamped placement scrolls its content instead of
                // truncating it — which is what `Placement::clamped` reports.
                height = rect.height,
                z = crate::components::overlay::BACKDROP_Z_INDEX + 1,
            ),
            {(request.content)()}
        }
    }
}

/// **The host must not decide.** Asserted against the source rather than by
/// reading it, because the drift is one convenient line: `place(request.placement)`
/// inside the render is shorter than reading a stored placement, and it silently
/// takes placement off the path `on_anchor_change` — and therefore the reposition
/// counter — watches.
///
/// A source assertion is a blunt instrument. It is the right blunt instrument
/// here: the property is syntactic, the file is short by design, and the failure
/// it guards produced no test failure at all when it happened.
#[cfg(test)]
mod host_purity {
    /// The rendered host, as text.
    /// **Read from this file, and it must stay that way.**
    ///
    /// The assertion previously read `component.rs`, and when r75 moved the host
    /// into this module it kept reading the old file — where the host no longer
    /// is. It **failed** rather than passing vacuously, which is the good
    /// outcome and the reason a source assertion should be sliced by a marker it
    /// would lose rather than by a filename it would keep: `split("pub fn
    /// AtPopoverHost")` returned nothing, and `nth(1)` on nothing is not a body
    /// that happens to contain no `place(`.
    const HOST: &str = include_str!("component_host.rs");

    #[test]
    fn the_host_render_never_calls_place() {
        // Sliced to the function's own closing brace, and comment lines dropped.
        // The first draft took everything after the `fn` and fired on this very
        // test's assertion message — prose about a forbidden call is not the call,
        // which is the same false positive the pending-questions gate had.
        let after = HOST
            .split("pub fn AtPopoverHost")
            .nth(1)
            .unwrap_or_default();
        let body = after.split("\n}\n").next().unwrap_or_default();
        let code: String = body
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect();
        assert!(
            !code.contains("place("),
            "AtPopoverHost calls `place(` — placement must be read from \
             `AtPopoverContext::resolved`, not derived during a render, or it \
             changes on paths the reposition counter cannot see",
        );
    }
}
