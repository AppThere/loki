// SPDX-License-Identifier: Apache-2.0

//! The page family's **list-level** manager verbs (Spec 08 T6.7): New,
//! Duplicate, Delete. Rename and Apply act on the style already open in the
//! form and live in [`super::page_form`]; these three act on the list, so they
//! sit next to it in [`super::page_browser`].
//!
//! # New and Duplicate were one button
//!
//! `new_page_style_button` seeded the created style from *the selected style's*
//! geometry when there was one, and from the default otherwise. That is
//! Duplicate wearing New's label — and it left New unreachable: with a style
//! selected there was no way to get a fresh default-geometry page style without
//! first deselecting, which the browser offers no way to do. Two verbs, one
//! control, and the one you could not name was the one you could not reach.
//!
//! They are separate now and each does only what its label says: New always
//! starts from [`PageLayout::default`], Duplicate always copies the selection
//! and is withheld when there is nothing to copy.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::{create_page_style, delete_page_style};
use loki_i18n::fl;

use super::StyleEditorSync;
use super::page_commit::commit;
use super::page_form::button_css;
use super::panel_data_page::{next_page_style_name, page_edit_target};
use crate::editing::state::DocumentState;

/// Creates a page style with `seed` geometry under the next free `PageStyleN`
/// and selects it, so the form opens on it and the user can rename it there.
///
/// Shared by New and Duplicate — the two differ only in where `seed` comes
/// from, which is the whole distinction between them and the reason it is a
/// parameter rather than a lookup inside.
fn create_and_select(
    doc_state: &Arc<Mutex<DocumentState>>,
    seed: PageLayout,
    mut editing_page_style: Signal<Option<String>>,
    sync: StyleEditorSync,
) {
    let Some(name) = next_page_style_name(doc_state) else {
        return;
    };
    if commit(doc_state, sync, |ldoc| {
        create_page_style(ldoc, &name, &seed)
    }) {
        editing_page_style.set(Some(name));
    }
}

/// "New": a page style with the **default** geometry, referenced by no section.
///
/// The browser lists unapplied styles for exactly this reason — a style that
/// appeared nowhere until it was in use could not be reached to put it in use.
///
/// # Touch target
///
/// A text button sharing [`button_css`]'s posture caveat.
pub(super) fn new_page_style_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    editing_page_style: Signal<Option<String>>,
    sync: StyleEditorSync,
) -> Element {
    let ds = Arc::clone(doc_state);
    rsx! {
        button {
            style: button_css(false),
            onclick: move |_| {
                create_and_select(&ds, PageLayout::default(), editing_page_style, sync);
            },
            { fl!("style-page-new") }
        }
    }
}

/// "Duplicate": a page style copying the selected one's geometry.
///
/// Withheld when nothing is selected — with no source to copy it would be a
/// second New, which is the conflation this pair exists to undo.
///
/// # Touch target
///
/// A text button sharing [`button_css`]'s posture caveat.
pub(super) fn duplicate_page_style_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<String>,
    editing_page_style: Signal<Option<String>>,
    sync: StyleEditorSync,
) -> Element {
    // Resolved now rather than in the handler: a selection naming a style the
    // document no longer has is nothing to duplicate, and withholding the
    // button says so better than a click that does nothing.
    let Some(seed) = selected
        .as_deref()
        .and_then(|s| page_edit_target(doc_state, s))
    else {
        return rsx! {};
    };
    let ds = Arc::clone(doc_state);
    rsx! {
        button {
            style: button_css(false),
            onclick: move |_| {
                create_and_select(&ds, seed.clone(), editing_page_style, sync);
            },
            { fl!("style-page-duplicate") }
        }
    }
}

/// "Delete": drops the selected style's **name**, leaving its pages as they are.
///
/// Withheld when nothing is selected. No confirmation prompt: the mutation goes
/// through the undo manager like every other edit here, and it changes no
/// geometry — the pages after a delete look exactly as they did before, which
/// is the property that makes it safe to offer bare. A prompt would be
/// protecting the user from a reversible rename.
///
/// # Touch target
///
/// A text button sharing [`button_css`]'s posture caveat.
pub(super) fn delete_page_style_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<String>,
    mut editing_page_style: Signal<Option<String>>,
    sync: StyleEditorSync,
) -> Element {
    let Some(name) = selected else {
        return rsx! {};
    };
    let ds = Arc::clone(doc_state);
    rsx! {
        button {
            style: button_css(false),
            onclick: move |_| {
                if commit(&ds, sync, |ldoc| delete_page_style(ldoc, &name)) {
                    // The selection names a style that no longer exists; leaving
                    // it set would keep the form open on nothing.
                    editing_page_style.set(None);
                }
            },
            { fl!("style-page-delete") }
        }
    }
}
