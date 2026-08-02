// SPDX-License-Identifier: Apache-2.0

//! The spelling menu as a popover consumer (Spec 08 T4.1, r63).
//!
//! # Why this is a component and the panel was not
//!
//! ADR-0013: a conditionally-mounted panel must be a component, because only a
//! component owns a hook scope. The spelling menu was `if cond {
//! plain_function(..) }`, which is one of the seven violations I-27 records — and
//! this migration is what forced the first fix, for a concrete reason rather
//! than for compliance: **handing the popover host a request is a signal write,
//! and a plain function has no `use_effect` to do it from.** Writing during a
//! render is the thing Dioxus asks you not to do.
//!
//! So the shape is: this component watches `spell_menu`, and on a change either
//! resolves a placement and hands over the content, or dismisses. It renders no
//! menu itself — `AtPopoverHost` does, at the app root.
//!
//! # Props by hand, as three other components in this crate do
//!
//! `#[component]` derives its props from the argument list and requires every
//! one to be `PartialEq`; `Arc<Mutex<DocumentState>>` is not. Same trade as
//! `CaretFollow`, `PageTile` and `ReflowDocView`: a hand-written props struct
//! whose `PartialEq` compares the `Arc` by pointer.
//!
//! # Touch target
//!
//! This component renders **nothing** — both the menu and its dismiss backdrop
//! are the host's, at the app root. The menu's rows carry the WCAG 2.5.8
//! minimum and are built in `editor_spell_panel`.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::components::popover::{
    KeyAction, OverlayKind, PopoverId, PopoverRequest, Role, use_popover_anchor,
};
use appthere_ui::{use_safe_area, use_window_size, window_size_signal};
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;

use crate::editing::state::DocumentState;
use crate::routes::editor::editor_spell::{SpellMenu, SpellSync};
use crate::routes::editor::editor_spell_panel::spell_menu_content;
use crate::routes::editor::editor_spell_place::spell_menu_placement;
use crate::routes::editor::editor_spell_rows::{
    SpellRowCtx, activate_spell_row, next_spell_row, prev_spell_row, spell_rows,
};

/// Identifies the spelling menu to the popover singleton rule.
///
/// A fixed id, content-independent: there is one spelling menu and opening it
/// again for a different word is the same popover moving, not a second one.
const SPELL_POPOVER_ID: PopoverId = PopoverId(0x5_9E11);

/// See the module docs for why these are hand-written.
#[derive(Clone, Props)]
pub(super) struct SpellPopoverProps {
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    pub(super) sync: SpellSync,
    pub(super) service: SpellService,
    pub(super) spell_menu: Signal<Option<SpellMenu>>,
    pub(super) is_language_panel_open: Signal<bool>,
    pub(super) spell_hover: Signal<Option<String>>,
    /// The editor container's scroll offset — one of the driver's two change
    /// sources (Spec 08 D-15). See the effect below for why the menu needs it.
    pub(super) scroll_offset: Signal<f32>,
}

impl PartialEq for SpellPopoverProps {
    fn eq(&self, other: &Self) -> bool {
        // `SpellService` is deliberately excluded: it is a handle to a shared
        // checker that lives for the session, so it cannot differ between two
        // renders of this component, and it implements no `PartialEq` to compare.
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.sync == other.sync
            && self.spell_menu == other.spell_menu
            && self.is_language_panel_open == other.is_language_panel_open
            && self.spell_hover == other.spell_hover
            && self.scroll_offset == other.scroll_offset
    }
}

/// Hands the spelling menu to the popover host.
// PascalCase for rsx; `#[component]` cannot be used here — see the module docs.
#[allow(non_snake_case)]
pub(super) fn SpellPopover(props: SpellPopoverProps) -> Element {
    // `use_popover_anchor`, not `use_popover`: this component is mounted behind
    // `if spell_menu.read().is_some()` in `editor_docked_panels`, so dismissing
    // the menu unmounts it — and the effect below is the only thing that ever
    // calls `dismiss`. Without the unmount cleanup this hook installs, `open`
    // stayed `Some` forever and the host's window-sized backdrop was left over
    // the whole application, swallowing every click while showing nothing (r66).
    let popover = use_popover_anchor(SPELL_POPOVER_ID);
    let window = use_window_size();
    let insets = use_safe_area();
    let spell_menu = props.spell_menu;
    let spell_hover = props.spell_hover;
    let scroll_offset = props.scroll_offset;
    // The container offset this menu was placed against. Captured in the open
    // effect rather than at mount: re-opening on a different word leaves this
    // component mounted, so a mount-time capture would go stale on the second
    // word and the menu would be repositioned by a delta it never travelled.
    let mut opened_at_scroll = use_signal(|| 0.0_f32);

    // The open write, in an effect rather than in the render. Repositioning is
    // the second effect below (r74) — this one runs on a *menu change* and must
    // not subscribe to the driver's change sources.
    use_effect(move || {
        let Some(anchor) = popover else {
            return;
        };
        let Some(menu) = spell_menu.read().clone() else {
            // Reached only if the signal clears while this component is still
            // mounted; the ordinary path is the unmount cleanup in
            // `use_popover_anchor`, because `editor_docked_panels` mounts this
            // behind the same condition.
            anchor.dismiss();
            return;
        };
        let doc_state = Arc::clone(&props.doc_state);
        let sync = props.sync;
        let service = props.service.clone();
        let is_language_panel_open = props.is_language_panel_open;
        // `peek`, not read: this effect must not subscribe to scroll, or opening
        // the menu would re-run on every scroll event and re-place from a fresh
        // baseline each time — which is the I-20 shape (a command subscribing to
        // the state it acts on) in a different module.
        opened_at_scroll.set(*scroll_offset.peek());
        // A second set of handles for the key path: `doc_state` and `service`
        // above are moved into `content`.
        let key_ctx = SpellRowCtx {
            doc_state: Arc::clone(&props.doc_state),
            sync,
            service: props.service.clone(),
        };
        anchor.open(
            PopoverRequest {
                // Stamped by the anchor with the id passed to
                // `use_popover_anchor`, so registration and open cannot disagree.
                id: SPELL_POPOVER_ID,
                // The click point is window-relative and the host's containing
                // block starts at the window origin, so no conversion — see
                // `editor_spell_place`. The viewport is the host's to fill.
                placement: spell_menu_placement(menu.anchor_x, menu.anchor_y),
                // Both layers are the host's now (r64) — one owner, one
                // lifetime, and this is the backdrop the outside-click path
                // consults.
                //
                // The r64 justification for it was wrong and is retracted: it
                // said the leftover backdrop at 1000 "competed directly" with
                // the host's 41 because the editor root creates no stacking
                // context. Blitz creates none anywhere — z-index sorts siblings
                // only — so the leftover, being inside `Router`, could never
                // have outranked a root sibling. See `popover::anchor_scope`.
                on_dismiss: Rc::new(move || {
                    let mut menu = spell_menu;
                    menu.set(None);
                }),
                on_outside_move: Some(Rc::new(move || {
                    // `Signal` is `Copy`, so a fresh binding inside the `Fn` gives
                    // the mutable handle a `Fn` closure cannot capture.
                    let mut hover = spell_hover;
                    if hover.peek().is_some() {
                        hover.set(None);
                    }
                })),
                kind: OverlayKind::Dismissible,
                role: Role::Menu,
                // **No anchor element.** This menu is anchored to a *point* —
                // the right-click — not to a control, so there is nothing to
                // return focus to and `dismiss_sequence` takes the branch that
                // moves none. Focus stays in the editor, which is where the
                // user was.
                anchor: None,
                on_key: Some(Rc::new(move |action: KeyAction| {
                    let mut hover = spell_hover;
                    let Some(menu) = spell_menu.peek().clone() else {
                        return;
                    };
                    let rows = spell_rows(&menu);
                    let current = hover.peek().clone();
                    let current = current.as_deref();
                    let moved = match action {
                        KeyAction::Next => next_spell_row(&rows, current),
                        KeyAction::Prev => prev_spell_row(&rows, current),
                        KeyAction::First => rows.first().copied(),
                        KeyAction::Last => rows.last().copied(),
                        KeyAction::Activate => {
                            // Only a row the *keyboard* can name is activated:
                            // `index_of` rejects a stale hover key, so Enter
                            // with nothing selected does nothing rather than
                            // guessing at the first row.
                            if let Some(key) = current
                                && let Some(row) = rows.iter().find(|r| r.key() == key)
                            {
                                activate_spell_row(
                                    *row,
                                    &menu,
                                    &key_ctx,
                                    spell_menu,
                                    is_language_panel_open,
                                );
                            }
                            None
                        }
                        // Typeahead over suggestions would compete with the
                        // suggestions themselves being words; left unhandled
                        // rather than half-implemented.
                        _ => None,
                    };
                    if let Some(row) = moved {
                        hover.set(Some(row.key()));
                    }
                })),
                content: Rc::new(move || {
                    spell_menu_content(
                        Arc::clone(&doc_state),
                        sync,
                        service.clone(),
                        spell_menu,
                        is_language_panel_open,
                        spell_hover,
                    )
                }),
            },
            window,
            insets,
        );
    });

    // ── The anchor driver (Spec 08 D-15) ────────────────────────────────────
    //
    // Event-driven, because there is no frame source: `scroll::animate` is an
    // animation clock (one thread per animation, 13 ticks, live only during a
    // smooth scroll), so a frame model would compare rects during Find and Go To
    // Page and never for a wheel, a drag or a resize.
    //
    // **Where the moved anchor comes from.** The menu is anchored to a word in
    // the document, and nothing re-reports that word's window position after the
    // click. But it does not need to be re-read: the word is fixed in *content*
    // space, so scrolling the container by Δ moves it by −Δ in window space, and
    // Δ is the difference between the current offset and the one captured at
    // open. That is arithmetic on a figure the editor already publishes, not a
    // second measurement of the same thing (L08-029).
    //
    // The resize half needs no anchor arithmetic at all: the word has not moved,
    // the *viewport* has, and `on_anchor_change` compares both halves precisely
    // so a resize cannot be mistaken for a scroll.
    use_effect(move || {
        let Some(anchor) = popover else {
            return;
        };
        let Some(menu) = spell_menu.peek().clone() else {
            return;
        };
        // Both change sources, read **inside** the closure — that is what
        // subscribes this effect to them. `use_window_size()` reads at render
        // time, so capturing its value here would leave the driver deaf to
        // resizes while looking correct; `window_size_signal` exists for exactly
        // that reason.
        let scrolled_by = scroll_offset() - *opened_at_scroll.peek();
        let window = window_size_signal().map(|s| *s.read());

        // The container's own visible height bounds how far the word can travel
        // before it has certainly left the scroll container. This is deliberately
        // a *bound* and not the container's window rect: computing that rect is
        // "container metrics plus known chrome", the arithmetic T4.1 exists to
        // delete. Erring toward `true` is the safe direction — the viewport half
        // of the judgement is `anchor_is_anchorable`'s, and it is not fooled.
        //
        // NOT ESTABLISHED: that a word scrolled behind the ribbon but still
        // inside the window is caught. It is not, by either half. The menu would
        // hold position pointing at content under the chrome until the word
        // leaves the window entirely. Left as a known bound rather than papered
        // over with a chrome constant.
        let still_in_container = window.is_none_or(|(_, h)| scrolled_by.abs() < h as f32);

        // Geometry only. The content closure and the callbacks stay as they were
        // registered at open — `reposition` takes a `PlacementRequest` precisely
        // so a driver cannot swap them by rebuilding the request.
        anchor.reposition(
            spell_menu_placement(menu.anchor_x, menu.anchor_y - scrolled_by),
            window,
            insets,
            still_in_container,
        );
    });

    // Renders nothing: both the menu and its backdrop are the host's, at the app
    // root. What is left here are the two effects above — which is the whole
    // reason this had to become a component.
    rsx! {}
}
