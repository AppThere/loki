// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Performing the dismissal sequence (Spec 08 T4.5).
//!
//! [`super::dismiss_order`] decides *what* happens and in what order. This is
//! the one place that turns a [`DismissStep`] into an action against the live
//! document — and, for the steps this stack cannot express, says so by name
//! instead of falling through a `_ => {}`.
//!
//! # Programmatic focus exists here only because a patch was written for it
//!
//! `RenderedElementBacking::set_focus` is a trait method with a `NotSupported`
//! default, and the vendored `dioxus-native-dom` backing did not override it —
//! so until r78 **no component in this workspace could move focus at all**, and
//! `focus_after_dismiss` was a decision with no possible caller rather than one
//! with a forgotten caller. The remedy was at the layer that owned the gap:
//! `MountedBackend::focus_node` routes to `BaseDocument::set_focus_to` /
//! `clear_focus`, exactly as `scroll_node_to` already routed scrolling. See
//! `docs/patches.md`.
//!
//! # One step remains unexpressible, and it is marked rather than silently
//! downgraded
//!
//! [`DismissStep::AdvanceFocusPastAnchor`] needs "focus the node *after* this
//! one", which the `bool` shape of `set_focus` cannot say; blitz-dom's
//! `focus_next_node` could serve it, but reaching it from here would mean
//! `appthere-ui` — the renderer-agnostic design system — taking a dependency on
//! the renderer crate. [`DismissStep::FallbackToAnchorContainer`] needs a handle
//! on the anchor's scroll container, which nothing carries.
//!
//! Both are therefore [`FocusOp::Unsupported`], which is an outcome the caller
//! matches on rather than a case that quietly does nothing. That distinction is
//! the whole of L08-050: a produced-but-unhandled outcome is a live
//! inconsistency, and only the marking separates parked from forgotten.
//! TODO(popover-focus-advance): see the tech-debt row in `CLAUDE.md`.

use std::rc::Rc;

use dioxus::prelude::*;

use super::dismiss_order::DismissStep;

/// What a [`DismissStep`] costs the wiring.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FocusOp {
    /// Focus the anchor element.
    FocusAnchor,
    /// Remove the popover. The caller's own business — this module does not
    /// own the signals.
    Unmount,
    /// This stack cannot express the step. Named, not skipped.
    Unsupported(Unsupported),
}

/// Which unexpressible step, so a log line and a test can tell them apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Unsupported {
    /// [`DismissStep::AdvanceFocusPastAnchor`].
    PastAnchor,
    /// [`DismissStep::FallbackToAnchorContainer`].
    AnchorContainer,
}

/// The action a step maps to.
///
/// Pure and exhaustive, so a new `DismissStep` is a compile error here rather
/// than a step that is produced and never performed.
#[must_use]
pub(crate) fn focus_op(step: DismissStep) -> FocusOp {
    match step {
        DismissStep::RestoreFocusToAnchor => FocusOp::FocusAnchor,
        DismissStep::AdvanceFocusPastAnchor => FocusOp::Unsupported(Unsupported::PastAnchor),
        DismissStep::FallbackToAnchorContainer => {
            FocusOp::Unsupported(Unsupported::AnchorContainer)
        }
        DismissStep::Unmount => FocusOp::Unmount,
    }
}

/// Runs the focus half of a dismissal, returning whether the caller should now
/// unmount.
///
/// `anchor` is the element the popover was opened from, `None` when the trigger
/// never reported a mounted handle.
///
/// # The unsupported steps degrade to the anchor, and say so
///
/// A user who pressed Tab out of a menu is better served landing **on** the
/// trigger — one extra Tab from where they wanted to be — than being left
/// wherever focus happened to sit, which after an unmount is the document root.
/// So the degradation is deliberate and bounded, and it is logged rather than
/// inferred from behaviour: a silent downgrade is indistinguishable from the
/// feature working (L08-050).
pub(crate) fn perform(steps: &[DismissStep], anchor: Option<&Rc<MountedData>>) -> bool {
    let mut unmount = false;
    for step in steps {
        match focus_op(*step) {
            FocusOp::FocusAnchor => focus(anchor),
            FocusOp::Unmount => unmount = true,
            FocusOp::Unsupported(which) => {
                tracing::debug!(
                    ?which,
                    "popover: focus step unsupported on this backing; \
                     focusing the anchor instead",
                );
                focus(anchor);
            }
        }
    }
    unmount
}

/// `set_focus(true)`, spawned because the backing round-trips to the event loop.
///
/// A missing handle is a no-op rather than a fallback to the document: focus
/// stays where the dismissing action left it, which is the same answer
/// [`super::FocusTarget::Unchanged`] gives and a better one than the root.
fn focus(anchor: Option<&Rc<MountedData>>) {
    let Some(anchor) = anchor.cloned() else {
        return;
    };
    spawn(async move {
        // **`Ok(())` means "posted", not "focused".** The backing is
        // fire-and-forget by design — the document lives on the event-loop side
        // — so treating success as confirmation would be an instrument reporting
        // on the adjacent quantity.
        //
        // The *error* is worth reporting, and it has exactly one cause worth
        // naming: `NotSupported` is what the trait's default returns, so it is
        // what this call produces the day the `set_focus` patch is dropped —
        // which is how a dioxus bump silently broke scrolling once already
        // (docs/patches.md). Nothing about the behaviour would say so; focus
        // would simply stop coming back.
        if let Err(err) = anchor.set_focus(true).await {
            tracing::warn!(
                ?err,
                "popover: the backing refused a focus request — if this is \
                 NotSupported, the dioxus-native-dom set_focus patch is missing",
            );
        }
    });
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
