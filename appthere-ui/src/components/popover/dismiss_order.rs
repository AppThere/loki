// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **When** focus moves on dismissal, as opposed to where (Spec 08 T4.1).
//!
//! `focus_after_dismiss` answers *where*. The ordering relative to unmounting is
//! invisible to it, lives entirely in wiring, and is exactly the kind of thing
//! that gets settled by whichever order happened to be typed first — so it is
//! decided here, with the reason, rather than left to a diff.
//!
//! # Restore first, then unmount
//!
//! Unmounting first leaves focus on a node that no longer exists. The platform
//! then moves focus to the document body, and there is a frame in which nothing
//! is focused — which assistive technology announces, so a keyboard user hears
//! the document title between pressing Escape and arriving back at the control
//! they started from. Restoring first means focus never rests on a removed node.
//!
//! # The case that makes it not merely an ordering
//!
//! `DismissCause::AnchorScrolledAway` asks for focus on an anchor that is, by
//! construction, no longer on screen — and may no longer be focusable at all if
//! its list virtualised the row away. Restoring to it is then a no-op, and the
//! ordering rule alone would leave focus nowhere.
//!
//! So the sequence carries a **fallback**, and the fallback is the anchor's
//! scroll container rather than the document: it is the nearest thing that still
//! exists, is focusable, and puts the reader near where they were.

use super::interaction::FocusTarget;

/// One step of the dismissal sequence, in the order a caller must perform them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DismissStep {
    /// Move focus to the anchor. Skipped when the target is `Unchanged`.
    RestoreFocusToAnchor,
    /// Move focus past the anchor in tab order — a menu's Tab exit.
    AdvanceFocusPastAnchor,
    /// Move focus to the anchor's scroll container, because the anchor itself is
    /// gone. Only ever follows a failed restore.
    FallbackToAnchorContainer,
    /// Remove the popover from the tree.
    Unmount,
}

/// The ordered steps for a dismissal.
///
/// `anchor_focusable` is the caller's answer to "can focus actually land on the
/// anchor right now" — false when a virtualised list has dropped the row, which
/// is reachable via `AnchorScrolledAway`.
///
/// Returns at most three steps, always ending in [`DismissStep::Unmount`].
#[must_use]
pub fn dismiss_sequence(target: FocusTarget, anchor_focusable: bool) -> Vec<DismissStep> {
    let mut steps = Vec::with_capacity(3);
    match target {
        FocusTarget::Anchor if anchor_focusable => steps.push(DismissStep::RestoreFocusToAnchor),
        FocusTarget::Anchor => steps.push(DismissStep::FallbackToAnchorContainer),
        FocusTarget::PastAnchor => steps.push(DismissStep::AdvanceFocusPastAnchor),
        FocusTarget::Unchanged => {}
    }
    steps.push(DismissStep::Unmount);
    steps
}

#[cfg(test)]
#[path = "dismiss_order_tests.rs"]
mod tests;
