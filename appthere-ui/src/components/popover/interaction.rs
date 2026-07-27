// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! How a popover responds to the keyboard, to dismissal and to scrolling
//! (Spec 08 T4.1).
//!
//! Pure decision tables, for the same reason `geometry` is pure: these are
//! answerable exactly, and a screen session can only tell you that *something*
//! felt wrong. Focus order, focus restoration and key routing are deterministic
//! — assert them rather than tabbing through by hand.
//!
//! # The four consumers are two interaction classes, and they want different
//! keyboards
//!
//! T4.2's Recent Documents actions and T7.1's status-bar overflow are **menus**:
//! a list of commands, arrows move between them, Tab leaves, typeahead jumps.
//! T5.2's colour picker and T5.4's zoom popover are **panels**: they contain a
//! field, a slider and a list, so Tab must cycle *between* those controls and
//! arrows belong to whichever one has focus.
//!
//! One keyboard serving both is wrong for one of them — arrows stolen from a
//! field, or Tab dismissing a picker mid-edit. So [`Role`] is a deliberate
//! parameter, and this is the *opposite* call from `geometry`'s absent
//! `Before`/`After` axis: there, no named consumer needed the second case, so
//! adding it would have been designing for nobody. Here two named consumers need
//! each class, and the cost of discovering that in Phase 5 is a redesign of the
//! primitive after two consumers already depend on its shape.
//!
//! # The focus trap is not a separate decision
//!
//! A menu must **not** trap: Tab is its exit. A panel must trap: Tab cycles
//! within it. That is one decision with two consequences, so [`Role::traps_focus`]
//! is *derived* rather than accepted as a second prop — a `Menu` that traps, or a
//! `Panel` that does not, are states this API cannot express.

/// Which interaction model a popover follows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Role {
    /// A list of commands: arrows move, Enter activates, Tab leaves, typeahead
    /// jumps. T4.2's entry actions, T7.1's overflow.
    #[default]
    Menu,
    /// A container of controls: Tab cycles within, arrows belong to the focused
    /// control, Escape closes. T5.2's colour picker, T5.4's zoom popover.
    Panel,
}

impl Role {
    /// Whether focus is confined to the popover while it is open.
    ///
    /// Derived, not configured — see the module docs.
    #[must_use]
    pub fn traps_focus(self) -> bool {
        matches!(self, Self::Panel)
    }
}

/// A key press, reduced to what this component distinguishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// Down arrow.
    Down,
    /// Up arrow.
    Up,
    /// Home.
    Home,
    /// End.
    End,
    /// Enter or Space.
    Activate,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Shift+Tab.
    ShiftTab,
    /// A printable character.
    Char(char),
}

/// What the popover does with a key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyAction {
    /// Move the active item forward.
    Next,
    /// Move the active item back.
    Prev,
    /// Move to the first item.
    First,
    /// Move to the last item.
    Last,
    /// Activate the active item and close.
    Activate,
    /// Close, restoring focus per [`focus_after_dismiss`].
    Dismiss,
    /// Close and let focus continue past the anchor.
    DismissAndAdvance,
    /// Move focus to the next control *inside* the popover.
    FocusNextControl,
    /// Move focus to the previous control inside the popover.
    FocusPrevControl,
    /// Jump to the item beginning with this character.
    Typeahead(char),
    /// Not ours — let the focused control have it.
    PassThrough,
}

/// Routes `key` for a popover of `role`.
///
/// The two rows that matter, and the reason [`Role`] exists:
///
/// | key | `Menu` | `Panel` |
/// | --- | --- | --- |
/// | arrows | move the active item | **pass through** to the focused control |
/// | `Tab` | close, focus moves on | **cycle** within the popover |
///
/// Arrows passed through is what stops a colour picker's slider from being
/// stolen; Tab cycling is what stops it from being dismissed mid-edit.
#[must_use]
pub fn route_key(role: Role, key: Key) -> KeyAction {
    match (role, key) {
        // Escape closes both, always. The one key with no per-role behaviour.
        (_, Key::Escape) => KeyAction::Dismiss,

        (Role::Menu, Key::Down) => KeyAction::Next,
        (Role::Menu, Key::Up) => KeyAction::Prev,
        (Role::Menu, Key::Home) => KeyAction::First,
        (Role::Menu, Key::End) => KeyAction::Last,
        (Role::Menu, Key::Activate) => KeyAction::Activate,
        (Role::Menu, Key::Tab | Key::ShiftTab) => KeyAction::DismissAndAdvance,
        (Role::Menu, Key::Char(c)) => KeyAction::Typeahead(c),

        (Role::Panel, Key::Tab) => KeyAction::FocusNextControl,
        (Role::Panel, Key::ShiftTab) => KeyAction::FocusPrevControl,
        // Everything else belongs to whichever control has focus: a text field
        // needs its own Home/End and its own characters, and a slider needs its
        // own arrows.
        (Role::Panel, _) => KeyAction::PassThrough,
    }
}

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
        DismissCause::Escape | DismissCause::Activated | DismissCause::AnchorScrolledAway => {
            FocusTarget::Anchor
        }
        DismissCause::TabOut => FocusTarget::PastAnchor,
        DismissCause::OutsideClick => FocusTarget::Unchanged,
    }
}

/// Where a scroll happened, relative to the anchor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollSource {
    /// A scroll container the anchor lives inside — the anchor moved.
    AnchorContainer,
    /// Anywhere else — the anchor did not move.
    Elsewhere,
}

/// What a scroll does to an open popover.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollResponse {
    /// Nothing: the anchor did not move.
    Ignore,
    /// Re-run placement against the anchor's new position.
    Reposition,
    /// Close.
    Dismiss,
}

/// How an open popover responds to a scroll.
///
/// # "Dismiss on scroll" is too blunt for T4.2
///
/// The Recent Documents menu anchors to an entry *inside a scrolling list*.
/// Under a flat dismiss-on-scroll rule, nudging a trackpad closes the menu — and
/// the list is the thing you scroll to reach entries in the first place.
///
/// The rule that serves both cases keys on **whether the anchor is still
/// visible**, not on whether a scroll occurred:
///
/// | source | anchor visible | response |
/// | --- | --- | --- |
/// | anchor's container | yes | reposition — the menu follows its entry |
/// | anchor's container | no | dismiss — anchoring to something off-screen is meaningless |
/// | elsewhere | either | ignore — the anchor did not move |
///
/// A nudge repositions; scrolling the entry away closes; scrolling an unrelated
/// pane does nothing. `Ignore` for `Elsewhere` is the part a flat rule gets
/// wrong in the other direction — an unrelated scroll should not close a menu
/// the user is reading.
#[must_use]
pub fn on_scroll(source: ScrollSource, anchor_still_visible: bool) -> ScrollResponse {
    match source {
        ScrollSource::Elsewhere => ScrollResponse::Ignore,
        ScrollSource::AnchorContainer if anchor_still_visible => ScrollResponse::Reposition,
        ScrollSource::AnchorContainer => ScrollResponse::Dismiss,
    }
}

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;
