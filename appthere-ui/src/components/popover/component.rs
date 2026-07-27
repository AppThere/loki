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

use dioxus::prelude::*;

use super::geometry::{place, Placement, PlacementRequest};
use super::wiring::PopoverId;

/// What a consumer hands the host: the content, where it belongs, and who owns
/// it.
///
/// `content` is an `Element` rather than a builder because the host's job is to
/// put it somewhere, not to know what it is — the same content serves a menu, a
/// panel and (via the tooltip component) a tooltip.
#[derive(Clone, PartialEq)]
pub struct PopoverRequest {
    /// Which popover this is, for the singleton rule.
    pub id: PopoverId,
    /// Where and how big — resolved by [`place`], never here.
    pub placement: PlacementRequest,
    /// What to render.
    pub content: Element,
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
    let Some(request) = ctx.open.read().clone() else {
        return rsx! {};
    };
    let Placement { rect, .. } = place(request.placement);
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
            {request.content}
        }
    }
}
