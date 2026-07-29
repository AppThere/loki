// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `Popover` host and its request type (Spec 08 T4.1).
//!
//! **This file wires. It does not decide.** Every decision lives in a sibling
//! module — [`super::geometry`], [`super::interaction`], [`super::dismiss_order`],
//! [`super::wiring`], [`super::host`]. Arithmetic or key matching appearing here
//! means one of those is missing a case.
//!
//! Its brevity is the check.

use std::rc::Rc;

use dioxus::prelude::*;

use super::geometry::{place, Placement, PlacementRequest};
use super::wiring::PopoverId;

/// What a consumer hands the host: the content, where it belongs, and who owns
/// it.
#[derive(Clone)]
pub struct PopoverRequest {
    /// Which popover this is, for the singleton rule.
    pub id: PopoverId,
    /// Where and how big — resolved by [`place`], never here.
    pub placement: PlacementRequest,
    /// What to render, as a **closure invoked during the host's render**.
    ///
    /// # Why not an `Element`
    ///
    /// An `Element` is a snapshot. Handed one, the host renders what the content
    /// looked like at the moment the popover opened, and nothing inside it ever
    /// updates: the spelling menu's row highlight is driven by a `spell_hover`
    /// signal, and a snapshot taken before the pointer moved cannot show it.
    /// Every consumer would then need an effect rebuilding the snapshot on each
    /// input change — four consumers, four chances to miss one, and the failure
    /// looks like a dead hover rather than a stale element.
    ///
    /// Invoking a closure inside the host's own render fixes it at the root:
    /// signal reads inside the closure subscribe **the host**, so the host
    /// re-renders when the content's inputs change. The consumer writes ordinary
    /// reactive code and does not think about it.
    pub content: Rc<dyn Fn() -> Element>,
}

/// Two requests are the same popover when they have the same identity and
/// placement.
///
/// The content closure is deliberately outside the comparison: closures are not
/// comparable, and it would be the wrong question anyway — the host re-invokes
/// it every render, so "has the content changed" is not a thing the host needs
/// to know.
impl PartialEq for PopoverRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.placement == other.placement
    }
}

/// The open popover, if any. Provided at the app root beside the backdrop.
#[derive(Clone, Copy)]
pub struct AtPopoverContext {
    /// `None` when nothing is open.
    pub open: Signal<Option<PopoverRequest>>,
}

/// Provides popover state. Call at the app root, before mounting
/// [`AtPopoverHost`].
#[must_use]
pub fn use_provide_popover() -> AtPopoverContext {
    let ctx = AtPopoverContext {
        open: use_signal(|| None),
    };
    use_context_provider(|| ctx);
    ctx
}

/// Reads the popover context, if one was provided.
#[must_use]
pub fn use_popover() -> Option<AtPopoverContext> {
    try_consume_context::<AtPopoverContext>()
}

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
    let window = crate::responsive::use_window_size();
    let insets = crate::use_safe_area();
    let Some(request) = ctx.open.read().clone() else {
        return rsx! {};
    };
    // **The host owns the viewport**, and the consumer's value is overwritten.
    //
    // The window and the safe area are properties of *this* coordinate space —
    // the host is the app root's child, so it is the thing that knows them. Four
    // consumers each reconstructing them is four chances to reconstruct them
    // differently, and the reconstruction available to a consumer is "container
    // metrics plus known chrome", which is the ~41px class of error T4.1 exists
    // to remove.
    //
    // An unmeasured window (`None`, or the `(0, 0)` before the first
    // measurement) leaves the request's own viewport alone rather than clamping
    // everything into a zero rect: on the first frame that is the difference
    // between a menu placed where the consumer asked and a menu at the origin.
    let mut placement = request.placement;
    if let Some((w, h)) = window.filter(|(w, h)| *w > 0.0 && *h > 0.0) {
        placement.viewport = super::geometry::Rect::new(
            insets.left,
            insets.top,
            (w as f32 - insets.left - insets.right).max(0.0),
            (h as f32 - insets.top - insets.bottom).max(0.0),
        );
    }
    let Placement { rect, .. } = place(placement);
    rsx! {
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
                z = super::super::BACKDROP_Z_INDEX + 1,
            ),
            {(request.content)()}
        }
    }
}
