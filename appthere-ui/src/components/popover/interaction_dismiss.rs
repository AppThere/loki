// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Why a popover closed, and where focus goes afterwards (Spec 08 T4.1).
//!
//! Split from `interaction.rs` at the 300-line ceiling (r69), on the same seam
//! `interaction_anchor` used: the key tables are about a keystroke arriving,
//! this is about the popover ending. Re-exported from `interaction` so every
//! existing path stays valid.

/// Why a popover closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DismissCause {
    /// Escape was pressed.
    Escape,
    /// An item was chosen.
    Activated,
    /// A click landed outside.
    OutsideClick,
    /// Tab moved focus out of a menu.
    TabOut,
    /// The anchor scrolled out of view.
    AnchorScrolledAway,
    /// The anchor's subtree was unmounted — a navigation, a tab close, the
    /// screen the anchor belonged to going away.
    ///
    /// # Root hosting created this cause
    ///
    /// Rendered beside its trigger, a popup was unmounted for free when its
    /// subtree was: no cause was needed. Hosted at the root and driven by a
    /// signal, its lifetime is **decoupled from its anchor's** — so opening a
    /// Recent Documents menu and then navigating from anywhere that is not the
    /// popup leaves the menu outliving the screen it belongs to.
    ///
    /// Neither [`Self::AnchorScrolledAway`] nor `wiring::IdentityCheck` covers
    /// it: both assume the list still exists and are asking where in it the
    /// anchor is. The anchor's own cleanup must raise this — wiring the pure
    /// modules cannot see, which is why it is a named cause rather than a note.
    AnchorUnmounted,
    /// The viewport changed enough that the popover would have to change
    /// **form** — anchored to modal, or back.
    ///
    /// # No producer today (r68), and kept deliberately
    ///
    /// This was raised when `present` returned a modal form and the viewport
    /// shrank past the anchored one. There is now **one form** — the modal was
    /// withdrawn because nothing implemented it — so a shrinking viewport
    /// repositions instead of closing, and nothing constructs this cause.
    ///
    /// # Why this is kept and `Presentation::Modal` was deleted
    ///
    /// The two look like opposite rulings on the same facts, and the difference
    /// is the one that generalises: **reachable-but-unimplemented is a defect;
    /// unreachable-and-marked is a deferral.**
    ///
    /// `Modal` was *produced* — two call sites received it and treated it
    /// incompatibly, one rendering nothing and one dismissing. A live
    /// inconsistency, doing damage, which is why it got the L08-043 treatment of
    /// being made unrepresentable. This variant is produced by nothing, so
    /// nothing can disagree about it; it costs its own row in the focus table and
    /// one test, and it holds a decision that would otherwise be remade from
    /// scratch.
    ///
    /// **The marking is the load-bearing part.** A reference-count sweep sees
    /// this exactly as it sees I-26's `reduced_motion` — referenced only in
    /// tests — and the only thing separating *parked* from *forgotten* is that
    /// this says which it is. `scripts/pending-questions.txt` carries the trigger
    /// so the distinction survives the person who made it.
    ///
    /// Kept rather than deleted because the *first consumer to implement a real
    /// modal fallback* needs exactly it, and because its focus answer is already
    /// decided and tested: focus returns to the anchor, since the anchor still
    /// exists and the user did not dismiss anything. Deleting it would discard a
    /// settled decision that has to be remade the moment T5.2's colour picker —
    /// whose SV square, hue strip and fields are tall — wants the fallback.
    ///
    /// The original argument, still the reason a *transform* is not the answer:
    /// a menu becoming a full-screen sheet under the user's hands is startling,
    /// and the two forms do not share a focus model. See `interaction_anchor`
    /// for why closing is not the answer either, now that there is no transform
    /// to avoid.
    PresentationChanged,
}

/// Where focus goes when a popover closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusTarget {
    /// Back to the control that opened it.
    Anchor,
    /// Onward, past the anchor, in tab order.
    PastAnchor,
    /// Leave focus wherever the dismissing action put it.
    Unchanged,
}

/// Where focus goes after a dismissal.
///
/// # Returning focus to the anchor is the point
///
/// Losing focus to the document root on close is the most common accessibility
/// defect in this component class: a keyboard user who opens a menu, presses
/// Escape and finds themselves at the top of the page has effectively been
/// ejected from their own task. It is fully determined by the cause, so it is
/// asserted here rather than left to a screen session that cannot check it.
///
/// The exceptions are the causes that *already* moved focus deliberately:
/// clicking elsewhere puts focus where the click landed, and Tab out of a menu
/// is a request to continue past the anchor rather than to return to it.
#[must_use]
pub fn focus_after_dismiss(cause: DismissCause) -> FocusTarget {
    match cause {
        // `PresentationChanged` joins these: the anchor still exists and the
        // user did not move focus themselves, so returning it there is both
        // available and correct — and re-opening from the anchor is exactly how
        // they get the form the new viewport calls for.
        DismissCause::Escape
        | DismissCause::Activated
        | DismissCause::AnchorScrolledAway
        | DismissCause::PresentationChanged => FocusTarget::Anchor,
        DismissCause::TabOut => FocusTarget::PastAnchor,
        // Both leave focus alone, for different reasons. An outside click put
        // focus where the user aimed it. An unmount has no anchor to return to
        // *and* is nearly always a navigation, which has already placed focus
        // on whatever replaced the screen — moving it again would fight that.
        DismissCause::OutsideClick | DismissCause::AnchorUnmounted => FocusTarget::Unchanged,
    }
}
