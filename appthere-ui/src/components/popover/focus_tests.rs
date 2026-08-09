// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The step-to-action mapping.
//!
//! `perform` itself needs a Dioxus runtime (it spawns), so what is covered is
//! [`super::focus_op`] — which is the decision. The value of these tests is
//! **which** steps are `Unsupported`: when the capability lands, this is the
//! file that has to change, so the deferral cannot quietly outlive its reason.

use super::{focus_op, FocusOp, Unsupported};
use crate::components::popover::{dismiss_sequence, DismissStep, FocusTarget};

/// The step the whole task was blocked on is performable.
#[test]
fn restoring_focus_to_the_anchor_is_a_real_action() {
    assert_eq!(
        focus_op(DismissStep::RestoreFocusToAnchor),
        FocusOp::FocusAnchor,
        "if this is ever Unsupported again, the `set_focus` patch has been \
         dropped — which is how the dioxus bump broke scrolling (docs/patches.md)",
    );
}

/// Unmount is the caller's, not this module's.
#[test]
fn unmount_is_handed_back_to_the_caller() {
    assert_eq!(focus_op(DismissStep::Unmount), FocusOp::Unmount);
}

/// **The deferral, pinned to the two steps it covers and no others.** Written as
/// equality rather than as `matches!(.., Unsupported(_))` so widening the gap —
/// a third step quietly joining them — is a failure here rather than a silent
/// loss of behaviour at the call site.
#[test]
fn exactly_two_steps_are_unsupported_and_these_are_they() {
    assert_eq!(
        focus_op(DismissStep::AdvanceFocusPastAnchor),
        FocusOp::Unsupported(Unsupported::PastAnchor),
    );
    assert_eq!(
        focus_op(DismissStep::FallbackToAnchorContainer),
        FocusOp::Unsupported(Unsupported::AnchorContainer),
    );
}

/// **The polarity that keeps the test above from meaning "nothing works"
/// (L08-045).** Two of the four steps are unsupported; if all four were, every
/// assertion about the deferral would still pass while dismissal moved no focus
/// at all — which is the state this task found the workspace in.
#[test]
fn not_every_step_is_unsupported() {
    let supported = [DismissStep::RestoreFocusToAnchor, DismissStep::Unmount]
        .into_iter()
        .filter(|s| !matches!(focus_op(*s), FocusOp::Unsupported(_)))
        .count();
    assert_eq!(supported, 2);
}

/// **The sequence a keyboard user actually produces is fully performable.**
/// Escape on a menu whose anchor is still mounted yields restore-then-unmount,
/// and neither step is a degradation — so the deferral above does not touch the
/// dominant path. This is the assertion that would fail if `dismiss_sequence`
/// and this mapping ever disagreed about which steps are ordinary.
#[test]
fn the_escape_path_needs_nothing_this_stack_cannot_do() {
    let steps = dismiss_sequence(FocusTarget::Anchor, true);
    assert_eq!(
        steps,
        vec![DismissStep::RestoreFocusToAnchor, DismissStep::Unmount],
    );
    for step in steps {
        assert!(
            !matches!(focus_op(step), FocusOp::Unsupported(_)),
            "{step:?} degrades on the commonest dismissal there is",
        );
    }
}

/// Every step a sequence can contain has a mapping — the exhaustiveness the
/// `match` gives, asserted over the *produced* set rather than over the enum, so
/// it covers a future step that `dismiss_sequence` emits.
#[test]
fn every_step_a_sequence_produces_has_a_mapping() {
    for target in [
        FocusTarget::Anchor,
        FocusTarget::PastAnchor,
        FocusTarget::Unchanged,
    ] {
        for focusable in [true, false] {
            let steps = dismiss_sequence(target, focusable);
            assert!(
                steps.contains(&DismissStep::Unmount),
                "{target:?}/{focusable} produced no Unmount, so the popover \
                 would stay open",
            );
            // `focus_op` is total by construction; calling it is the check.
            for step in steps {
                let _ = focus_op(step);
            }
        }
    }
}
