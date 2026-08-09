// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ordering, asserted as the failure a screen-reader user would hear.

use super::super::interaction::FocusTarget;
use super::{dismiss_sequence, DismissStep};

/// **Focus must move before the popover leaves the tree.**
///
/// Unmounting first leaves focus on a removed node, the platform drops it to the
/// document body, and assistive technology announces the document title between
/// Escape and arriving back at the control. Asserted as an ordering rather than
/// as a step list, because the list could be right and the order wrong.
#[test]
fn focus_is_restored_before_the_popover_unmounts() {
    let steps = dismiss_sequence(FocusTarget::Anchor, true);
    let restore = steps
        .iter()
        .position(|s| *s == DismissStep::RestoreFocusToAnchor)
        .expect("focus must be restored");
    let unmount = steps
        .iter()
        .position(|s| *s == DismissStep::Unmount)
        .expect("the popover must unmount");
    assert!(
        restore < unmount,
        "unmount at {unmount} precedes restore at {restore}: focus would rest on \
         a removed node and the user would hear the document announced",
    );
}

/// The case the ordering rule alone does not cover: the anchor is gone, so
/// restoring to it is a no-op and focus would land nowhere.
#[test]
fn an_unfocusable_anchor_falls_back_rather_than_losing_focus() {
    let steps = dismiss_sequence(FocusTarget::Anchor, false);
    assert_eq!(steps[0], DismissStep::FallbackToAnchorContainer);
    assert_eq!(steps.last(), Some(&DismissStep::Unmount));
    assert!(
        !steps.contains(&DismissStep::RestoreFocusToAnchor),
        "restoring to an anchor that cannot take focus is a silent no-op",
    );
}

/// Every sequence ends by unmounting, and none is empty — a dismissal that did
/// not remove the popover would leave it open with focus moved away.
#[test]
fn every_sequence_ends_in_unmount() {
    for target in [
        FocusTarget::Anchor,
        FocusTarget::PastAnchor,
        FocusTarget::Unchanged,
    ] {
        for focusable in [true, false] {
            let steps = dismiss_sequence(target, focusable);
            assert_eq!(
                steps.last(),
                Some(&DismissStep::Unmount),
                "{target:?} / focusable {focusable} did not end in unmount",
            );
            assert!(steps.len() <= 2, "unexpected extra steps: {steps:?}");
        }
    }
}

/// `Unchanged` must not touch focus at all — an outside click already put focus
/// where the user aimed it, and moving it again would yank it away.
#[test]
fn an_outside_click_leaves_focus_alone_and_only_unmounts() {
    for focusable in [true, false] {
        assert_eq!(
            dismiss_sequence(FocusTarget::Unchanged, focusable),
            vec![DismissStep::Unmount],
        );
    }
}
