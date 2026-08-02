// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tying an open popover's lifetime to its consumer's (Spec 08 T4.1, r66).
//!
//! # The primitive named this hazard and then did not wire it
//!
//! [`super::interaction::DismissCause::AnchorUnmounted`] has documented, since
//! the primitive was written, that root hosting decouples the popup's lifetime
//! from its anchor's and that **the anchor's own cleanup must close it**. It had
//! no caller. A decision nothing calls is indistinguishable from one nobody made,
//! and this is what it cost:
//!
//! `SpellPopover` is mounted behind `if spell_menu.read().is_some()`. Choosing a
//! suggestion set that signal to `None`; the consumer unmounted; and its
//! `use_effect` — the only thing that ever called `dismiss` — went with it. The
//! host's `open` stayed `Some` indefinitely. The *menu* still disappeared,
//! because the content closure reads the same signal and renders nothing, so what
//! remained was the backdrop alone: transparent, window-sized, at the app root.
//! From that moment every click in the application hit it, and clicking it ran
//! `on_dismiss`, which set an already-`None` signal and changed nothing. The
//! editor, the scrollbar and the tab bar were all dead, with nothing on screen to
//! explain why.
//!
//! # Why this is a hook and not a line in each consumer
//!
//! Four consumers are planned. "Remember to dismiss on unmount" is exactly the
//! kind of obligation three of them will meet and one will not, and the one that
//! does not produces a dead application rather than a visible glitch — so the
//! remedy has to make the wrong thing unavailable rather than documented
//! (L08-043).
//!
//! [`use_popover_anchor`] installs the cleanup as a condition of getting the
//! opener. [`AtPopoverContext::open_resolved`] is `pub(crate)`, so a consumer
//! cannot reach it any other way: opening a popover has, by construction, already
//! registered the thing that closes it.

use dioxus::prelude::*;

use super::component::{use_popover, AtPopoverContext, PopoverRequest};
use super::dismiss_order::dismiss_sequence;
use super::interaction::{
    focus_after_dismiss, note_event, on_anchor_change, AnchorResponse, DismissCause,
};
use super::wiring::{dismiss_on_unmount, PopoverId};

impl AtPopoverContext {
    /// Closes the popover **only if `id` is the one currently open**.
    ///
    /// See [`dismiss_on_unmount`] for why it is keyed rather than unconditional:
    /// the singleton rule means another popover may have replaced this one, and
    /// closing that one instead would be an intermittent vanishing menu.
    ///
    /// Reads and writes fallibly. This runs from a drop handler, which on
    /// application teardown can execute after the root scope's signals are gone;
    /// `peek`/`set` would panic there, turning a clean exit into a crash on quit.
    /// It is also `peek` rather than `read` on purpose — a drop handler is not a
    /// reactive context and must not subscribe.
    pub fn dismiss_if_open(mut self, id: PopoverId) {
        let Ok(open) = self.open.try_peek() else {
            return; // runtime is tearing down; nothing left to dismiss
        };
        let currently_open = open.as_ref().map(|request| request.id);
        drop(open);
        if !dismiss_on_unmount(currently_open, id) {
            return;
        }
        if let Ok(mut slot) = self.open.try_write() {
            *slot = None;
        }
        if let Ok(mut slot) = self.resolved.try_write() {
            *slot = None;
        }
    }

    /// Closes `id` **with a reason**, running the focus sequence that reason
    /// calls for (T4.5).
    ///
    /// # Every dismissal that moves focus goes through here
    ///
    /// [`Self::dismiss_if_open`] is the unmount path and answers
    /// [`DismissCause::AnchorUnmounted`], whose focus target is `Unchanged` — so
    /// it needs none of this. Every *other* cause does, and giving each of them
    /// its own restore call would be four consumers × five causes of chances to
    /// order it wrong. The order is the thing [`dismiss_sequence`] exists to fix,
    /// and it is only fixed if there is one performer.
    ///
    /// The consumer's own `on_dismiss` is what actually removes the popover — it
    /// clears the state the consumer is mounted behind — so it is called for the
    /// [`super::DismissStep::Unmount`] step, **after** the focus steps. That is
    /// the ordering `dismiss_order` was written for: restoring focus to a node
    /// that has already been removed leaves focus on the document body, and a
    /// screen reader announces the document title between Escape and arriving
    /// back at the control.
    pub(crate) fn dismiss_with(self, id: PopoverId, cause: DismissCause) {
        let Ok(open) = self.open.try_peek() else {
            return;
        };
        // Keyed, for the same reason `dismiss_if_open` is: the singleton rule
        // means another popover may have replaced this one since the event that
        // is now closing it was scheduled.
        let Some(request) = open.as_ref().filter(|r| r.id == id).cloned() else {
            return;
        };
        drop(open);
        let target = focus_after_dismiss(cause);
        // "Can focus land on the anchor" is, at this layer, "did the trigger
        // ever report a mounted handle". The document-side check — the node may
        // have been removed since — is in the patch's own handler, which is the
        // only place that can see it.
        let steps = dismiss_sequence(target, request.anchor.is_some());
        if super::focus::perform(&steps, request.anchor.as_ref()) {
            (request.on_dismiss)();
        }
    }
}

/// A consumer's handle on the popover host, bound to the id it opens under.
///
/// The only way to open a popover from outside this crate, and it exists only
/// from [`use_popover_anchor`] — so the unmount cleanup is not something a
/// consumer can forget to install.
#[derive(Clone, Copy)]
pub struct PopoverAnchor {
    ctx: AtPopoverContext,
    id: PopoverId,
}

impl PopoverAnchor {
    /// Opens `request`, resolving its placement against the measured window.
    ///
    /// `request.id` is **stamped with this anchor's id**. A consumer that
    /// registered under one id and opened under another would install a cleanup
    /// that never fires, which is precisely the defect this type exists to close;
    /// stamping makes the two agree by construction rather than by care.
    pub fn open(
        self,
        request: PopoverRequest,
        window: Option<(f64, f64)>,
        insets: crate::SafeAreaInsets,
    ) {
        let mut request = request;
        request.id = self.id;
        self.ctx.open_resolved(request, window, insets);
    }

    /// Re-places this popover against an anchor that may have moved.
    ///
    /// **This is the driver** (Spec 08 D-15). Call it from an effect that reads
    /// the change sources — the scroll container's metrics and the window size —
    /// with `request` describing where the anchor is *now* and
    /// `still_in_container` covering what geometry cannot see.
    ///
    /// # Event-driven, because there is no frame source
    ///
    /// `scroll::animate` is an animation clock, not a tick: one thread per
    /// animation, 13 ticks, live only during a smooth scroll. Wired to it this
    /// would never run for a wheel, a drag, a resize or a reflow. See
    /// [`super::interaction::anchor::counter`] for what that changes about the
    /// instrument.
    ///
    /// # It does not write unless the placement changed, and that is load-bearing
    ///
    /// `Signal::set` notifies unconditionally — it does not compare. Writing on
    /// every event would re-render the host on every event, and the host's
    /// re-render can re-enter this comparison; the cycle would terminate only
    /// because the values stop differing, which is a coincidence of the input
    /// settling rather than a guarantee.
    ///
    /// So: [`on_anchor_change`] returns `Ignore` when nothing moved and nothing
    /// is written, and a `Reposition` is compared against what is stored before
    /// either signal is touched. The reads here are **peeks**, never reactive
    /// reads, for the same reason `ViewportController` separates observation from
    /// command (L08-019): an effect that subscribed to what it writes is the loop
    /// under a different name.
    ///
    /// # Both signals move together
    ///
    /// `open`'s stored `placement` is what the next comparison uses as
    /// `previous`. Updating `resolved` without it would compare every subsequent
    /// event against the placement the popover *opened* at, so a settled popover
    /// would report a reposition forever — a live version of the loop this is
    /// built to avoid. They are written in one place for that reason (L08-043).
    /// # It takes a `PlacementRequest`, not a `PopoverRequest`, and that is a fix
    ///
    /// The first draft took the whole request. A reposition then *replaces* what
    /// is stored — including `content` and the callbacks — so a consumer whose
    /// driver rebuilt the request slightly differently would silently swap the
    /// menu's content on the first scroll. The content is an `Rc<dyn Fn>`,
    /// excluded from `PopoverRequest`'s `PartialEq` by design, so neither the
    /// comparison here nor a test of it would have noticed; the symptom would be
    /// a menu that empties when you scroll.
    ///
    /// Taking only the geometry makes that unrepresentable: the stored request's
    /// content and callbacks are preserved by construction, and a driver has
    /// nothing to get wrong (L08-043).
    pub fn reposition(
        mut self,
        placement: super::geometry::PlacementRequest,
        window: Option<(f64, f64)>,
        insets: crate::SafeAreaInsets,
        still_in_container: bool,
    ) {
        let Ok(open) = self.ctx.open.try_peek() else {
            return;
        };
        // Only this popover's own driver may move it; the singleton rule means
        // another may have replaced it since the effect was scheduled.
        let Some(previous) = open.as_ref().filter(|r| r.id == self.id).cloned() else {
            return;
        };
        drop(open);

        // Everything but the geometry is carried over from what is stored.
        let mut current = previous.clone();
        current.placement = placement;
        current.placement.viewport =
            super::geometry::usable_viewport(window, insets, placement.viewport);
        note_event();
        match on_anchor_change(previous.placement, current.placement, still_in_container) {
            // The whole idempotence obligation, in one arm: nothing moved, so
            // nothing is written and the host does not re-render.
            AnchorResponse::Ignore => {}
            AnchorResponse::Dismiss => self.dismiss_with(DismissCause::AnchorScrolledAway),
            AnchorResponse::Reposition(placement) => {
                if previous.placement == current.placement {
                    return;
                }
                if let Ok(mut slot) = self.ctx.open.try_write() {
                    *slot = Some(current);
                }
                if let Ok(mut slot) = self.ctx.resolved.try_write() {
                    *slot = Some(placement);
                }
            }
        }
    }

    /// Closes this popover, and only this one.
    ///
    /// Keyed on the anchor's id — see [`dismiss_on_unmount`]. A consumer whose
    /// popover was already replaced under the singleton rule closes nothing here,
    /// rather than closing the replacement.
    pub fn dismiss(self) {
        self.ctx.dismiss_if_open(self.id);
    }

    /// Closes this popover **for a stated reason**, restoring focus accordingly.
    ///
    /// This is the dismissal a user performs — Escape, Tab, choosing an item.
    /// [`Self::dismiss`] is the one the *machinery* performs, where there is no
    /// keyboard user to return focus to.
    pub fn dismiss_with(self, cause: DismissCause) {
        self.ctx.dismiss_with(self.id, cause);
    }
}

/// A [`PopoverAnchor`] with this consumer's unmount cleanup already installed.
///
/// `id` is the [`PopoverId`] this component opens under; when the component
/// unmounts, whatever it has open under that id is closed.
///
/// Returns `None` when the application has not called
/// [`super::component::use_provide_popover`], on the same degrade-quietly rule as
/// [`use_popover`].
///
/// # Touch target
///
/// Not applicable — this is a hook and renders nothing.
#[must_use]
pub fn use_popover_anchor(id: PopoverId) -> Option<PopoverAnchor> {
    let ctx = use_popover();
    // Unconditional, as every hook must be: `ctx` is captured by value and the
    // `None` case is handled inside, rather than by skipping the hook.
    use_drop(move || {
        if let Some(ctx) = ctx {
            ctx.dismiss_if_open(id);
        }
    });
    ctx.map(|ctx| PopoverAnchor { ctx, id })
}
