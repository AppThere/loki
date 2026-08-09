// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The shared overlay primitive (Spec 08 T4.1).
//!
//! # Invariant: the component wires, it does not decide
//!
//! Every decision lives in one of four pure modules — [`geometry`] (placement,
//! flip, shift, clamp), [`interaction`] (key routing, focus target),
//! [`interaction_anchor`](interaction::anchor) (scroll and resize response) and
//! [`dismiss_order`] (the order focus and unmount happen in). **Arithmetic or key
//! matching appearing in the component is a signal that a module is missing a
//! case, not that the component needs logic.**
//!
//! That is what stops the four consumers — T4.2's entry menu, T5.2's colour
//! picker, T5.4's zoom popover, T7.1's status overflow — from each growing their
//! own variant of a decision already made, which is the failure the
//! "scope for four consumers, not two" instruction guards against.
//!
//! It is checkable by reading: the component should be short enough that its
//! brevity is itself the check.
//!
//! # Two hazards the pure modules cannot see
//!
//! - **Reposition loops.** [`interaction::on_anchor_change`] compares rects for
//!   exact float equality, which is correct. Under the event driver (r74) the
//!   loop that remains is *reactive* rather than temporal — a write feeding back
//!   into the driver's own effect — so the instrument is an **invariant** rather
//!   than a threshold: one event runs one comparison yielding at most one
//!   reposition, so [`interaction::repositions`] exceeding [`interaction::events`]
//!   is arithmetically impossible without re-entry, and it **warns in
//!   production** because the cause is a signal graph, which tests do not
//!   reproduce.
//! - **The anchor is not "outside".** [`wiring::is_outside_dismiss`] excludes it,
//!   or clicking the trigger dismisses and the trigger's own handler reopens —
//!   the control stops toggling and both handlers look correct.
//! - **One popover at a time.** [`wiring::open_response`] enforces it, since the
//!   reposition counter is process-wide and a focus restoration needs one
//!   candidate anchor. Opening a second dismisses the first.
//! - **Lifetime.** Root hosting decouples the popup's life from its anchor's
//!   subtree, so the anchor's cleanup must raise
//!   [`interaction::DismissCause::AnchorUnmounted`] — otherwise a navigation
//!   leaves a menu outliving the screen it belongs to. Nothing else covers it:
//!   `AnchorScrolledAway` and [`wiring::IdentityCheck`] both assume the list
//!   still exists. **Wired by [`anchor_scope::use_popover_anchor`] (r66) — and
//!   unwired it cost the whole application.** A conditionally-mounted consumer
//!   took its own `dismiss` call away when it unmounted, leaving the host's
//!   window-sized backdrop up with nothing visible inside it, swallowing every
//!   click in the editor, the scrollbar and the tab bar. The cause was documented
//!   here from the start and had no caller, which is why the remedy is a hook
//!   that installs the cleanup as a condition of getting the context.
//! - **Root layer order.** [`host::RootLayer`] — `z-index` cannot arbitrate
//!   between two children of the positioned root, so DOM order does, and a
//!   backdrop painting over its own popup reads as a dead menu.
//! - **Tooltips share the host, not the [`interaction::Role`].** See
//!   [`host`]: T4.3's Open-button tooltip is the same out-of-flow child in the
//!   same clipping container as T4.2's menu.
//! - **A band between two modules.** [`geometry::place`] clamps a partly-visible
//!   anchor's overlay into the viewport; [`interaction::on_anchor_change`]
//!   dismisses when the anchor is not visible — and while "visible" was a
//!   caller's `bool`, neither module owned the case in between.
//!   [`interaction::anchor_is_anchorable`] now decides the viewport half in one
//!   place, leaving the caller only what geometry cannot see.
//! - **Identity, checked before geometry.** [`wiring::on_anchor_identity`]. A
//!   virtualised list recycling a different row into the same node leaves the
//!   anchor rect unchanged, so the geometric comparison correctly says `Ignore`
//!   and the menu acts on the wrong document.
//!
//! # Where this must mount, and why it is not a z-index question
//!
//! Measured against the tree rather than assumed, because it decides the
//! component's mount point and that is expensive to change once four consumers
//! exist:
//!
//! 1. **There is no portal and no top layer.** Nothing in the workspace
//!    implements one, and `position: fixed` collapses to `absolute` in this Blitz
//!    stack (`components::overlay`).
//! 2. **`AtBackdropHost` hosts the *backdrop* at the app's positioned root, not
//!    the popup.** Its own docs say the popup "stays wherever the requester
//!    rendered it".
//! 3. **The binding constraint is clipping, not stacking.** The Home screen's
//!    Recent Documents list carries `overflow-y: auto` (and `overflow: hidden` on
//!    its expanded sibling), and an out-of-flow child is clipped by an ancestor's
//!    overflow — **no `z-index` escapes a clip**.
//!
//! So T4.2 is the first consumer that needs the *popup itself* hosted at the
//! root, not merely its backdrop: rendered in place it would be correctly
//! placed and invisible below the fold of its own list, which is
//! indistinguishable from being placed wrong. The component therefore renders
//! into a root-mounted host, on the `AtBackdropHost` pattern, rather than beside
//! its anchor — and [`geometry::place`] already works in viewport coordinates,
//! which is the coordinate space a root-mounted host needs.
//! - **Dismissal ordering.** Focus must move *before* the popover unmounts —
//!   see [`dismiss_order`], which returns the sequence rather than leaving the
//!   order to whichever line was typed first.

pub mod anchor_scope;
pub mod component;
pub mod dismiss_order;
pub mod focus;
pub mod geometry;
pub mod host;
pub mod interaction;
pub mod key_event;
pub mod presentation;
pub mod wiring;

pub use anchor_scope::{use_popover_anchor, PopoverAnchor};
pub use component::{
    use_popover, use_provide_popover, AtPopoverContext, AtPopoverHost, OverlayKind, PopoverRequest,
};
pub use dismiss_order::{dismiss_sequence, DismissStep};
pub use geometry::{place, usable_viewport, Align, Placement, PlacementRequest, Rect, Side};
pub use host::RootLayer;
pub use interaction::{
    anchor_is_anchorable, events, focus_after_dismiss, note_event, on_anchor_change, repositions,
    reset_repositions, route_key, AnchorResponse, DismissCause, FocusTarget, Key, KeyAction, Role,
};
pub use key_event::key_from_parts;
pub use presentation::{present, MENU_ROW_HEIGHT_PX, MIN_ANCHORED_HEIGHT_PX, MIN_ANCHORED_MENU_PX};
pub use wiring::{
    dismiss_on_unmount, is_outside_dismiss, on_anchor_identity, open_response, AnchorKey,
    IdentityCheck, OpenResponse, PopoverId,
};
