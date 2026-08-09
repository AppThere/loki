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

#[path = "interaction_anchor.rs"]
mod anchor;

#[path = "interaction_dismiss.rs"]
mod dismiss;
pub use anchor::{
    anchor_is_anchorable, events, note_event, on_anchor_change, repositions, reset_repositions,
    AnchorResponse,
};
pub use dismiss::{focus_after_dismiss, DismissCause, FocusTarget};

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
    /// A non-interactive overlay: no controls, no focus, no keys. T4.3's
    /// Open-button tooltip.
    ///
    /// # The variant the first real dispatcher forced (r78)
    ///
    /// There were two roles while `route_key` had no caller, and the tooltip was
    /// filed under `Panel` on the reasoning that it "handles nothing itself".
    /// Wiring the dispatch made that false in the one way that matters:
    /// `route_key(Panel, Tab)` is `FocusNextControl`, which **consumes** the key
    /// and hands it to an `on_key` the tooltip does not supply — so Tab while a
    /// tooltip is showing would go nowhere at all. A tooltip is a keyboard trap
    /// the moment it is treated as a container of controls.
    ///
    /// It is also the role that must **not** take focus on mount: a tooltip
    /// appears because the pointer rested on a button, and stealing focus from
    /// whatever the user was typing in is a defect no amount of key routing
    /// fixes. See [`Self::takes_focus`].
    Tooltip,
}

impl Role {
    /// Whether focus is confined to the popover while it is open.
    ///
    /// Derived, not configured — see the module docs.
    #[must_use]
    pub fn traps_focus(self) -> bool {
        matches!(self, Self::Panel)
    }

    /// Whether the overlay should take focus when it opens.
    ///
    /// A menu and a panel are things the user went to, so focus follows; a
    /// tooltip appeared under a resting pointer, so it must not. Derived rather
    /// than configured, on the same rule as [`Self::traps_focus`]: "an overlay
    /// that grabs focus and routes no keys" is not a state anyone wants, so it
    /// should not be expressible.
    #[must_use]
    pub fn takes_focus(self) -> bool {
        !matches!(self, Self::Tooltip)
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
    /// Backspace.
    ///
    /// Added when the zoom menu grew a typed field (Spec 08 T5.4): a menu that
    /// accepts typed characters and has no way to remove one lets a reader
    /// correct a typo only by starting again. It is in the vocabulary rather
    /// than special-cased in that menu because `route_key` is exhaustive per
    /// role — a new `Key` is a compile error in every role, which is how each
    /// gets an answer instead of a default.
    Backspace,
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
    /// Remove the last typed character, for a menu that accepts typed input.
    Erase,
    /// Not ours — let the focused control have it.
    PassThrough,
}

impl KeyAction {
    /// Whether the popover **consumed** this key, so the wiring must stop
    /// propagation.
    ///
    /// # What `route_key` cannot say
    ///
    /// It says what the popover does; it cannot say what happens next. If
    /// Escape propagates, closing a menu *also* cancels whatever the editor
    /// beneath does on Escape — one keypress, two effects, and the second is
    /// invisible until someone loses an edit. Enter is the same shape: it
    /// activates a menu item and then reaches a form or the editor below.
    ///
    /// Derived from the action rather than configured beside it, for the reason
    /// [`Role::traps_focus`] is: "handled but not consumed" is not a state
    /// anyone wants, so it should not be expressible. Everything except
    /// [`Self::PassThrough`] consumes — and `PassThrough` must *not*, or a
    /// panel's own controls would never see their keys.
    #[must_use]
    pub fn consumes(self) -> bool {
        !matches!(self, Self::PassThrough)
    }

    /// The dismissal this action **is**, if the host owns it.
    ///
    /// # Why this is a function and not two arms in the host
    ///
    /// Closing is the host's business, so the host answers `Dismiss` and
    /// `DismissAndAdvance` itself and never forwards them to `on_key`. That is
    /// correct and it is *invisible*: a consumer writing
    /// `KeyAction::Dismiss => …` in its own `on_key` gets an arm that compiles,
    /// reads as handled, and never runs. The zoom menu did exactly that — its
    /// `Dismiss` arm cleared the typed field, so one Escape left the field open
    /// with a stale value owning the keyboard for the rest of the session, and
    /// the arm that would have prevented it was sitting right there (r93).
    ///
    /// One fact, one derivation (L08-029): the host asks this, and
    /// `a_consumers_on_key_never_sees_a_dismissal` asserts the same set from the
    /// other side. A consumer that needs to know a dismissal happened has
    /// [`super::PopoverRequest::on_dismiss`], which fires for **every** cause —
    /// including the outside click and the anchor leaving, which no key routes.
    #[must_use]
    pub fn dismissal_cause(self) -> Option<DismissCause> {
        match self {
            Self::Dismiss => Some(DismissCause::Escape),
            Self::DismissAndAdvance => Some(DismissCause::TabOut),
            _ => None,
        }
    }
}

/// Routes `key` for a popover of `role`.
///
/// The two rows that matter, and the reason [`Role`] exists:
///
/// | key | `Menu` | `Panel` | `Tooltip` |
/// | --- | --- | --- | --- |
/// | arrows | move the active item | **pass through** to the focused control | pass through |
/// | `Tab` | close, focus moves on | **cycle** within the popover | pass through |
/// | `Escape` | close | close | **pass through** |
///
/// Arrows passed through is what stops a colour picker's slider from being
/// stolen; Tab cycling is what stops it from being dismissed mid-edit.
#[must_use]
pub fn route_key(role: Role, key: Key) -> KeyAction {
    match (role, key) {
        // A tooltip handles nothing and **consumes** nothing — including Escape,
        // which belongs to whatever the user is actually working in. First,
        // because the Escape row below is otherwise role-blind.
        (Role::Tooltip, _) => KeyAction::PassThrough,

        // Escape closes the two interactive roles, always.
        (Role::Menu | Role::Panel, Key::Escape) => KeyAction::Dismiss,

        (Role::Menu, Key::Down) => KeyAction::Next,
        (Role::Menu, Key::Up) => KeyAction::Prev,
        (Role::Menu, Key::Home) => KeyAction::First,
        (Role::Menu, Key::End) => KeyAction::Last,
        (Role::Menu, Key::Activate) => KeyAction::Activate,
        (Role::Menu, Key::Tab | Key::ShiftTab) => KeyAction::DismissAndAdvance,
        (Role::Menu, Key::Char(c)) => KeyAction::Typeahead(c),
        (Role::Menu, Key::Backspace) => KeyAction::Erase,

        (Role::Panel, Key::Tab) => KeyAction::FocusNextControl,
        (Role::Panel, Key::ShiftTab) => KeyAction::FocusPrevControl,
        // Everything else belongs to whichever control has focus: a text field
        // needs its own Home/End and its own characters, and a slider needs its
        // own arrows.
        //
        // Spelled out rather than written `(Role::Panel, _)`. A catch-all would
        // absorb a future `Key` variant into `PassThrough` silently, which is the
        // same expiring guarantee that made the deleted
        // `every_key_is_routed_for_every_role` worthless — a promise that holds
        // only until someone adds a variant, and then keeps looking kept. With
        // both roles exhaustive, a new `Key` is a **compile error** in this
        // function, so the property needs no test at all.
        (Role::Panel, Key::Down | Key::Up | Key::Home | Key::End) => KeyAction::PassThrough,
        (Role::Panel, Key::Activate) => KeyAction::PassThrough,
        (Role::Panel, Key::Char(_)) => KeyAction::PassThrough,
        (Role::Panel, Key::Backspace) => KeyAction::PassThrough,
    }
}

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;
