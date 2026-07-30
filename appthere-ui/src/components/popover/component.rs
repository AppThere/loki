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

use super::geometry::{Placement, PlacementRequest};
use super::wiring::PopoverId;

/// What a consumer hands the host: the content, where it belongs, and who owns
/// it.
#[derive(Clone)]
pub struct PopoverRequest {
    /// Which popover this is, for the singleton rule.
    pub id: PopoverId,
    /// Where and how big — resolved by [`super::geometry::place`], never here.
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

/// The open popover, if any, **and its resolved placement**.
///
/// # Two signals, because one of them must not be recomputed on render
///
/// `resolved` holds the placement that was decided, and the host renders it
/// verbatim. It is deliberately *not* derived in the host's render, and the
/// reason is an instrument rather than tidiness.
///
/// A first draft called [`super::geometry::place`] inside [`AtPopoverHost`]. That is wrong twice
/// over. It puts a **decision in the component**, which this module's invariant
/// forbids — but worse, it puts placement on a path the reposition counter cannot
/// see. `PopoverRequest::content` is a closure invoked during the host's render,
/// so every signal the content reads subscribes *the host*; the spelling menu's
/// `spell_hover` changes on every pointer move over the list, each change
/// re-renders the host, and each re-render would recompute placement. Placement
/// could then churn on every mouse move while
/// `idle_frames_perform_no_repositions` stayed green, because
/// [`super::interaction::on_anchor_change`] — where the counter lives — was never
/// involved.
///
/// That is L9-011 one layer in: an instrument on the right question, watching the
/// wrong path. The fix is to leave exactly one way for placement to change —
/// somebody writes `resolved` — so the counter and the behaviour cannot diverge.
#[derive(Clone, Copy)]
pub struct AtPopoverContext {
    /// `None` when nothing is open.
    pub open: Signal<Option<PopoverRequest>>,
    /// The placement the host renders. Written by whoever opens the popover and
    /// by the per-frame driver; **never** derived during a render.
    pub resolved: Signal<Option<Placement>>,
}

impl AtPopoverContext {
    /// Opens `request`, resolving its placement against the measured window.
    ///
    /// # The one place a viewport is filled in
    ///
    /// The window and the safe area belong to the host's coordinate space, so the
    /// consumer supplies an anchor and preferences and this fills the rest. Four
    /// consumers reconstructing window geometry is four chances to differ, and
    /// the reconstruction available to a consumer — container metrics plus known
    /// chrome — is the ~41px class of error T4.1 exists to remove.
    ///
    /// Sharing it with the per-frame driver is the point: both paths call
    /// [`super::presentation::present`] on a request filled the same way, so a
    /// reposition cannot land somewhere the open never would (L08-028).
    ///
    /// A `Modal` outcome stores no placement — the anchored form does not fit, and
    /// the consumer renders its own full-screen presentation. That is a change of
    /// form, not a suppression; see [`super::presentation`].
    pub fn open_resolved(
        mut self,
        request: PopoverRequest,
        window: Option<(f64, f64)>,
        insets: crate::SafeAreaInsets,
    ) {
        let mut filled = request.clone();
        filled.placement.viewport =
            super::geometry::usable_viewport(window, insets, request.placement.viewport);
        let placement = match super::presentation::present(filled.placement) {
            super::presentation::Presentation::Anchored(p) => Some(p),
            super::presentation::Presentation::Modal => None,
        };
        self.open.set(Some(filled));
        self.resolved.set(placement);
    }

    /// Closes whatever is open.
    pub fn dismiss(mut self) {
        self.open.set(None);
        self.resolved.set(None);
    }
}

/// Provides popover state. Call at the app root, before mounting
/// [`AtPopoverHost`].
#[must_use]
pub fn use_provide_popover() -> AtPopoverContext {
    let ctx = AtPopoverContext {
        open: use_signal(|| None),
        resolved: use_signal(|| None),
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
    // Read, never computed. See `AtPopoverContext::resolved`: deriving placement
    // here would take it off the one path the reposition counter watches.
    let Some(Placement { rect, .. }) = *ctx.resolved.read() else {
        return rsx! {};
    };
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
    const HOST: &str = include_str!("component.rs");

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
