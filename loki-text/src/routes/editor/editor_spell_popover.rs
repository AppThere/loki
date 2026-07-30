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

use appthere_ui::components::popover::{PopoverId, PopoverRequest};
use appthere_ui::{use_popover, use_safe_area, use_window_size};
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;

use crate::editing::state::DocumentState;
use crate::routes::editor::editor_spell::{SpellMenu, SpellSync};
use crate::routes::editor::editor_spell_panel::spell_menu_content;
use crate::routes::editor::editor_spell_place::spell_menu_placement;

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
    }
}

/// Hands the spelling menu to the popover host.
// PascalCase for rsx; `#[component]` cannot be used here — see the module docs.
#[allow(non_snake_case)]
pub(super) fn SpellPopover(props: SpellPopoverProps) -> Element {
    let popover = use_popover();
    let window = use_window_size();
    let insets = use_safe_area();
    let spell_menu = props.spell_menu;
    let spell_hover = props.spell_hover;

    // The one write, and it is in an effect rather than in the render.
    //
    // Resolved **once per menu change** rather than per frame: a window resize or
    // an editor scroll after the menu opens is the per-frame driver's job
    // (`popover::interaction::on_anchor_change`), which is not wired yet. Until it
    // is, the menu holds the position it opened at — which is the pre-migration
    // behaviour, so the migration does not regress it while not yet fixing it.
    use_effect(move || {
        let Some(ctx) = popover else {
            return;
        };
        let Some(menu) = spell_menu.read().clone() else {
            ctx.dismiss();
            return;
        };
        let doc_state = Arc::clone(&props.doc_state);
        let sync = props.sync;
        let service = props.service.clone();
        let is_language_panel_open = props.is_language_panel_open;
        ctx.open_resolved(
            PopoverRequest {
                id: SPELL_POPOVER_ID,
                // The click point is window-relative and the host's containing
                // block starts at the window origin, so no conversion — see
                // `editor_spell_place`. The viewport is the host's to fill.
                placement: spell_menu_placement(menu.anchor_x, menu.anchor_y),
                // Both layers are the host's now (r64). Keeping the backdrop
                // here put it at `z-index: 1000` inside an editor root that
                // creates no stacking context, so it painted over a menu the
                // host had placed correctly — every suggestion click dismissing
                // rather than choosing.
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

    // Renders nothing: both the menu and its backdrop are the host's, at the app
    // root. What is left here is the effect above — which is the whole reason
    // this had to become a component.
    rsx! {}
}
