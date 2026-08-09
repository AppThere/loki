// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where out-of-flow overlays mount, and in what order (Spec 08 T4.1).
//!
//! # Every out-of-flow overlay goes through a root host — including tooltips
//!
//! An out-of-flow child is clipped by an ancestor's `overflow`, and no `z-index`
//! escapes a clip. The Home screen's Recent Documents section is
//! `overflow-y: auto`, so **T4.3's Open-button tooltip is the same out-of-flow
//! child in the same clipping container** as T4.2's menu. A tooltip rendered in
//! place is clipped exactly as the spelling menu is today — a third instance of
//! the defect, shipped in the phase that fixed the first two.
//!
//! **Decision, and its retraction (r78).** This read: *a tooltip is not a
//! [`super::Role`] — its keyboard model is degenerate, it never takes focus, so
//! "what does Down do" has no answer, and giving it a `Role` would put an arm in
//! `route_key` that can never run.*
//!
//! The premise was right and the conclusion was backwards, and it took the first
//! real dispatcher (T4.5) to show which. A tooltip does not *want* a keyboard —
//! but it still **reaches** `route_key`, because the host routes every overlay it
//! renders and the request must name some role. Filed under `Panel`, the
//! degenerate model became an active one: `Tab` routed to `FocusNextControl`,
//! which *consumes* the key and hands it to an `on_key` a tooltip does not
//! supply, so Tab went nowhere while a tooltip was showing. The host's
//! `autofocus` compounded it by pulling focus out of whatever the pointer's owner
//! was typing in.
//!
//! So the arm that "can never run" was the one running, and
//! [`super::Role::Tooltip`] exists to say *pass everything through, take no
//! focus* in the one place both the router and the host read. What the tooltip
//! shares is unchanged: placement, the anchor-change response, and this host. The
//! failure mode is still implementing it **without** the host, which is why the
//! host is stated here rather than at T4.3.
//!
//! # The two root layers must be ordered, and only DOM order can order them
//!
//! `position: fixed` collapses to `absolute` and there is no top layer, so
//! `z-index` cannot arbitrate between two siblings of the positioned root: DOM
//! order decides. Two things now mount there — `AtBackdropHost` (outside-click
//! capture) and the overlay host — and if the backdrop paints *over* the popup,
//! the popup is visible and unclickable, which reads as a dead menu rather than
//! as a stacking bug.
//!
//! [`RootLayer`] states the order as data so it is checked rather than
//! discovered.

/// A layer mounted directly in the app's positioned root, in paint order.
///
/// Declared as an ordered enum rather than left to the order the `rsx!` happens
/// to list, because the failure — a backdrop painting over the popup it exists
/// to serve — presents as an unclickable menu rather than as a z-order problem.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum RootLayer {
    /// Outside-click capture. Must paint **below** the overlay and above all
    /// ordinary content.
    Backdrop,
    /// Popovers and tooltips. Must paint above the backdrop, or the overlay it
    /// belongs to cannot be clicked.
    Overlay,
}

impl RootLayer {
    /// Mount position in the root container, ascending. A host with a lower
    /// value must appear **earlier** in the root's children.
    #[must_use]
    pub fn mount_order(self) -> u8 {
        match self {
            Self::Backdrop => 0,
            Self::Overlay => 1,
        }
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
