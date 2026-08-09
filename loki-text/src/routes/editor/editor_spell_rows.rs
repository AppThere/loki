// SPDX-License-Identifier: Apache-2.0

//! The spelling menu's rows: which ones exist, in what order, and what each is
//! called (Spec 08 T4.5).
//!
//! # Why this exists as a table rather than as the renderer's structure
//!
//! The rows were built inline in `editor_spell_panel`, with their identity
//! carried by ad-hoc key strings — `format!("sug-{i}")`, `"add"`, `"ignore"`,
//! `"lang"` — written once at each row and once again at each
//! `is_hovered(spell_hover, ..)` call. That was fine while the only consumer of
//! a key was the pointer, because a typo produced a row that simply never
//! highlighted.
//!
//! The keyboard makes it load-bearing: Enter has to run *the action belonging to
//! the highlighted key*, and the highlighted key comes from walking the rows in
//! order. A key the navigator produces and the renderer does not draw is an
//! invisible selection; a key the renderer draws and the navigator skips is a
//! row Down cannot reach. Both are silent.
//!
//! So the order and the names live here, derived from the same two facts the
//! renderer branches on — `misspelled` and whether there are suggestions — and
//! both sides read them (L08-029).

use std::sync::{Arc, Mutex};

use dioxus::prelude::{Signal, WritableExt};
use loki_app_shell::spell::SpellService;

use super::editor_spell::{SpellMenu, SpellSync, add_to_dictionary, ignore_word, replace_word};
use crate::editing::state::DocumentState;

/// One row of the spelling menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SpellRow {
    /// The *n*th suggestion; choosing it replaces the word.
    Suggestion(usize),
    /// Add the word to the user dictionary.
    Add,
    /// Ignore the word for this session.
    Ignore,
    /// Open the language panel.
    Language,
}

impl SpellRow {
    /// The row's key, as used by `spell_hover` and by the row's own style.
    #[must_use]
    pub(super) fn key(self) -> String {
        match self {
            Self::Suggestion(i) => format!("sug-{i}"),
            Self::Add => "add".to_string(),
            Self::Ignore => "ignore".to_string(),
            Self::Language => "lang".to_string(),
        }
    }
}

/// The rows `menu` shows, in the order they are drawn.
///
/// The two conditions mirror `spell_menu_content`'s exactly: a correctly-spelled
/// word gets neither suggestions nor the add/ignore pair, and a misspelled word
/// with no suggestions gets the actions but no list. The language row is
/// unconditional, which is why it is the one row the keyboard can always reach.
#[must_use]
pub(super) fn spell_rows(menu: &SpellMenu) -> Vec<SpellRow> {
    let mut rows = Vec::with_capacity(menu.suggestions.len() + 3);
    if menu.misspelled {
        rows.extend((0..menu.suggestions.len()).map(SpellRow::Suggestion));
        rows.push(SpellRow::Add);
        rows.push(SpellRow::Ignore);
    }
    rows.push(SpellRow::Language);
    rows
}

/// The row after `current`, wrapping; `None` enters at the first row.
#[must_use]
pub(super) fn next_spell_row(rows: &[SpellRow], current: Option<&str>) -> Option<SpellRow> {
    let at = current.and_then(|key| index_of(rows, key));
    match at {
        Some(i) => rows.get((i + 1) % rows.len()).copied(),
        None => rows.first().copied(),
    }
}

/// The row before `current`, wrapping; `None` enters at the **last** row, so one
/// Up press reaches the bottom of the menu.
#[must_use]
pub(super) fn prev_spell_row(rows: &[SpellRow], current: Option<&str>) -> Option<SpellRow> {
    let at = current.and_then(|key| index_of(rows, key));
    match at {
        Some(0) | None => rows.last().copied(),
        Some(i) => rows.get(i - 1).copied(),
    }
}

/// Where `key` sits in `rows`, if it is still there.
///
/// `None` covers the case the pointer creates: `spell_hover` holds a key from
/// the row the pointer last crossed, and re-opening the menu on a different word
/// can leave a key that names no current row. Treating that as "no selection" is
/// what makes the first arrow press land at an end rather than nowhere.
fn index_of(rows: &[SpellRow], key: &str) -> Option<usize> {
    rows.iter().position(|row| row.key() == key)
}

/// What activating a row needs, bundled so the keyboard handler's capture list
/// stays readable.
///
/// Cloned rather than borrowed because it is captured by an `Rc<dyn Fn>` that
/// outlives the effect that builds it.
#[derive(Clone)]
pub(super) struct SpellRowCtx {
    pub doc_state: Arc<Mutex<DocumentState>>,
    pub sync: SpellSync,
    pub service: SpellService,
}

/// Runs `row`'s action and closes the menu.
///
/// **The same three functions the rendered rows' `onclick` closures call.** They
/// are reached from here rather than duplicated, so Enter on a suggestion and a
/// click on it are the same edit — the property that makes "the menu has a
/// keyboard" a true statement rather than a second, similar menu.
pub(super) fn activate_spell_row(
    row: SpellRow,
    menu: &SpellMenu,
    ctx: &SpellRowCtx,
    mut spell_menu: Signal<Option<SpellMenu>>,
    mut is_language_panel_open: Signal<bool>,
) {
    match row {
        SpellRow::Suggestion(i) => {
            // Bounds-checked rather than indexed: `rows` was derived from this
            // same menu, but the two reads are separated by the keypress, and a
            // panic in a UI callback takes the window with it.
            let Some(replacement) = menu.suggestions.get(i).cloned() else {
                return;
            };
            replace_word(&ctx.doc_state, ctx.sync, menu, &replacement);
        }
        SpellRow::Add => add_to_dictionary(
            &ctx.doc_state,
            ctx.sync.cursor_state,
            &ctx.service,
            &menu.word,
        ),
        SpellRow::Ignore => ignore_word(
            &ctx.doc_state,
            ctx.sync.cursor_state,
            &ctx.service,
            &menu.word,
        ),
        SpellRow::Language => {
            spell_menu.set(None);
            is_language_panel_open.set(true);
            return;
        }
    }
    spell_menu.set(None);
}

#[cfg(test)]
#[path = "editor_spell_rows_tests.rs"]
mod tests;
