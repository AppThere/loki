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

#[path = "component_host.rs"]
mod host_impl;
pub use host_impl::AtPopoverHost;

use super::geometry::{Placement, PlacementRequest};
use super::interaction::{KeyAction, Role};
use super::wiring::PopoverId;

/// What a consumer hands the host: the content, where it belongs, and who owns
/// it.
#[derive(Clone)]
pub struct PopoverRequest {
    /// Which popover this is, for the singleton rule.
    pub id: PopoverId,
    /// Where and how big — resolved by [`super::geometry::place`], never here.
    pub placement: PlacementRequest,
    /// Called when a click lands outside the popover.
    ///
    /// # The backdrop belongs to the host — and the r64 reason for it was wrong
    ///
    /// One owner and one lifetime is the real reason: the backdrop is what the
    /// outside-click path consults, and a backdrop outliving the popup it belongs
    /// to is the r66 failure.
    ///
    /// **Retracted.** r64 said the spelling menu's leftover backdrop at
    /// `z-index: 1000` "competed directly" with this host's 41 because the editor
    /// root creates no stacking context. Blitz creates none *anywhere*:
    /// `paint_children` is each parent's own layout children, sorted by
    /// `z_index()` among **siblings only**, painted by walking that list and
    /// hit-tested by walking it in reverse. A descendant's 1000 never meets a root
    /// sibling's 41 — the entire `Router` subtree loses to any root sibling with a
    /// higher z, at any value. So the predicted symptom could not have occurred,
    /// and the one that did was a lifetime defect; see [`super::anchor_scope`].
    pub on_dismiss: Rc<dyn Fn()>,
    /// Called when the pointer moves outside the popover, where a consumer needs
    /// it.
    ///
    /// Blitz dispatches no `mouseleave` and honours no CSS `:hover`, so a menu
    /// that tints the row under the pointer has no other way to learn the pointer
    /// left. Optional because only hover-tinting consumers need it — discovered
    /// by migrating the one that does.
    pub on_outside_move: Option<Rc<dyn Fn()>>,
    /// Whether an outside click dismisses this overlay — and therefore whether
    /// the host renders a backdrop to catch one.
    ///
    /// # A tooltip with a backdrop makes the application unclickable
    ///
    /// The backdrop is a transparent, **window-sized** click-catcher. That is
    /// exactly right for a menu, whose outside-click dismissal it implements, and
    /// catastrophic for a tooltip: a tooltip is dismissed by the pointer leaving
    /// its anchor, so its backdrop would capture every click in the application
    /// for as long as the pointer rests on an icon — the r66 failure with a
    /// different cause and no unmount to end it.
    ///
    /// Stated as an enum rather than a `bool` because the two are different
    /// *kinds* of overlay rather than one with a flag, and because a `bool` at a
    /// call site reads as "backdrop: false" — a rendering detail — rather than as
    /// "this is not dismissed by clicking", which is the decision.
    pub kind: OverlayKind,
    /// Which interaction model this overlay follows, for [`super::route_key`].
    ///
    /// A menu takes the arrows to move its active item; a panel passes them to
    /// whatever control has focus. Carried on the request rather than inferred,
    /// because the primitive cannot see what the content is.
    pub role: Role,
    /// What to do with a key the popover **consumed but did not handle itself**.
    ///
    /// # The host cannot move a selection it cannot see
    ///
    /// `content` is an opaque closure, so `Next`/`Prev`/`First`/`Last`/
    /// `Activate`/`Typeahead` are decisions the host can *route* and cannot
    /// *perform* — only the consumer knows what its items are. `Dismiss` and
    /// `DismissAndAdvance` are the host's, because closing is the one action it
    /// owns.
    ///
    /// So the split is: [`super::route_key`] decides, the host performs what it
    /// can, and this receives the rest. A consumer that ignores it gets a menu
    /// that closes on Escape and does not arrow — which is a smaller failure
    /// than a host guessing at content it does not understand.
    pub on_key: Option<Rc<dyn Fn(KeyAction)>>,
    /// The element focus returns to when this popover closes (T4.5).
    ///
    /// # Held by the host, not by the consumer, because the consumer unmounts
    ///
    /// Restoring focus is the *last* thing a dismissal does, and by then the
    /// consumer that owns the trigger may already be gone — the r66 failure in
    /// miniature. The host outlives every consumer, so the handle lives on the
    /// request it already holds.
    ///
    /// `None` when the trigger has not reported a mounted handle yet, which is
    /// a real state: `onmounted` fires after the first render. It is what
    /// [`super::dismiss_sequence`]'s `anchor_focusable` reads, so a popover
    /// opened without one takes the fallback branch rather than silently
    /// skipping the restore.
    pub anchor: Option<Rc<MountedData>>,
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

/// What dismisses an overlay, and therefore what the host must mount for it.
///
/// See [`PopoverRequest::kind`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverlayKind {
    /// A menu or panel. An outside click dismisses it, so the host mounts a
    /// backdrop to capture that click.
    Dismissible,
    /// A tooltip. Driven entirely by the pointer over its anchor, so **no
    /// backdrop** — see [`PopoverRequest::kind`] for what one would cost.
    PointerDriven,
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
    /// # `pub(crate)`: opening is reachable only through the anchor (r66)
    ///
    /// Consumers go through [`super::anchor_scope::PopoverAnchor::open`], which
    /// exists only from [`super::anchor_scope::use_popover_anchor`], which
    /// installs the unmount cleanup. Opening and closing had different owners
    /// once, and a consumer that unmounted took its own `dismiss` call with it —
    /// leaving the host's backdrop over a dead application. Making the opener
    /// unreachable without the closer is what stops the other three consumers
    /// repeating it (L08-043).
    pub(crate) fn open_resolved(
        mut self,
        request: PopoverRequest,
        window: Option<(f64, f64)>,
        insets: crate::SafeAreaInsets,
    ) {
        // **The singleton rule, wired (r77).** `open_response` decided this and
        // had no caller, and the gap was live: opening a second popover replaced
        // the first in `open` without telling the first's *consumer*, so that
        // consumer kept its own state set. Concretely — open a Recent row's menu,
        // then hover the Open button: the tooltip replaces the menu on screen
        // while `menu_open` stays `Some`, and the next click on ⋮ toggles that
        // stale state off instead of opening. **The button reads as dead for one
        // click**, which is the defect class this primitive exists to remove.
        //
        // `AlreadyOpen` deliberately does *not* dismiss: re-opening the same
        // popover for a different word is that popover moving, and running the
        // dismiss would fire the consumer's own close handler mid-open.
        let previous = self.open.peek().clone();
        let response = super::wiring::open_response(previous.as_ref().map(|r| r.id), request.id);
        if let super::wiring::OpenResponse::DismissThenOpen(_) = response {
            if let Some(previous) = previous {
                (previous.on_dismiss)();
            }
        }
        let mut filled = request.clone();
        filled.placement.viewport =
            super::geometry::usable_viewport(window, insets, request.placement.viewport);
        // Always a placement (r68): `present` clamps an overlay too small to use
        // up to the floor rather than returning a modal form nothing implements.
        // While it could, `resolved` stayed `None` here and the host rendered
        // nothing — a right-click that produced no menu and no way to tell why.
        let placement = super::presentation::present(filled.placement);
        self.open.set(Some(filled));
        self.resolved.set(Some(placement));
    }

    // There is deliberately **no unkeyed `dismiss`** (r66). One existed, and the
    // singleton rule makes it a foot-gun: a consumer calling it after another
    // popover replaced its own would close somebody else's menu, intermittently.
    // Every dismissal goes through `dismiss_if_open`, which is keyed — including
    // the per-frame driver's `AnchorScrolledAway` when that lands.
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
