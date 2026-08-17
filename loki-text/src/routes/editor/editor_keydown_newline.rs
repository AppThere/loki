// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Newline-bearing text payloads become **paragraph** breaks, not literal
//! newlines. Called by [`super::editor_keydown::make_keydown_handler`] in place
//! of [`handle_character_key`], which it delegates to for the common case.
//!
//! # Why a keystroke can arrive as text containing a newline
//!
//! On Android the app is a bare `NativeActivity`, which offers the IME no
//! `InputConnection`. The soft keyboard therefore cannot edit a text buffer and
//! instead **synthesises key events**, which winit resolves through the input
//! device's `KeyCharacterMap`. The virtual keyboard's map gives `KEYCODE_ENTER`
//! the base character `U+000A`, so winit takes the `KeyMapChar::Unicode` arm and
//! produces `Key::Character("\n")` — the `NamedKey::Enter` arm is only reached
//! when the map yields *no* character. Nothing between there and the editor
//! normalises it: `blitz-shell`, `blitz-dom` and `dioxus-native-dom` all pass
//! the key through unchanged.
//!
//! The result was that Enter on the soft keyboard inserted `U+000A` into the
//! paragraph's text run, which Parley renders as a line break — while the same
//! key on a physical keyboard arrives as `Key::Enter` and splits the paragraph.
//!
//! A second producer exists on desktop: `blitz-shell`'s IME-commit path wraps a
//! whole committed string as one `Key::Character`, so a multi-line commit lands
//! here too. Both are handled by the same rule.
//!
//! # The rule
//!
//! **A newline arriving as text means "new paragraph".** No keyboard has a key
//! that inserts a literal `U+000A` *as text* — a deliberate soft line break is a
//! separate gesture (Shift+Enter in Word, which this editor does not yet bind;
//! see the note in [`super::editor_keydown_enter`]'s neighbourhood). So this is
//! a normalisation, not a platform workaround, and it is deliberately **not**
//! `cfg`-gated to Android: the desktop commit path produces the same payload and
//! deserves the same meaning.
//!
//! Physical-keyboard input is untouched — it arrives as `Key::Enter` and never
//! reaches this module.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use super::editor_keydown_enter::handle_enter_key;
use super::editor_keydown_text::handle_character_key;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::DocumentState;

/// Routes the Enter key: **Shift+Enter inserts a line break**, plain Enter
/// splits the paragraph.
///
/// The two live together because they are one decision, and because the
/// normalisation above depends on it: a bare newline arriving *as text* is
/// promoted to a paragraph split precisely because a deliberate line break has
/// its own gesture. If that gesture ever moves (a different chord, a toolbar
/// button), both halves have to move together or the promotion starts eating
/// the very input it was meant to distinguish itself from.
///
/// A line break is inserted as `U+000A` through the ordinary text path, which
/// is what the CRDT stores for [`Inline::LineBreak`] — so it occupies one caret
/// position, replaces a selection, and takes a revision mark like any typed
/// character, all without a second mutation path.
///
/// Soft keyboards cannot send Shift+Enter, so on Android this arm is
/// effectively plain-Enter only — which is the correct behaviour there.
///
/// [`Inline::LineBreak`]: loki_doc_model::content::inline::Inline::LineBreak
#[allow(clippy::too_many_arguments)] // mirrors `handle_enter_key`'s arity
pub(super) fn enter_or_line_break(
    shift: bool,
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    if shift {
        handle_character_key(
            "\n".to_owned(),
            focus,
            loro_doc,
            doc_state,
            cursor_state,
            undo_manager,
            can_undo,
            can_redo,
        );
    } else {
        handle_enter_key(
            focus,
            loro_doc,
            doc_state,
            cursor_state,
            undo_manager,
            can_undo,
            can_redo,
        );
    }
}

/// Whether a committed text payload carries any newline form.
///
/// `\r` counts: a `KeyCharacterMap` may give `KEYCODE_ENTER` a carriage return
/// rather than a line feed, and treating that as ordinary text would insert an
/// invisible control character.
#[must_use]
pub(super) fn carries_newline(text: &str) -> bool {
    text.contains('\n') || text.contains('\r')
}

/// Splits a payload into the runs of text that sit *between* its newlines.
///
/// `n` newlines yield `n + 1` segments, so the caller emits one paragraph split
/// between each adjacent pair. A payload that is exactly one newline yields two
/// empty segments — one split and no text, which is precisely the Enter key.
///
/// CRLF is folded first so that a Windows-style break yields one split, not two.
#[must_use]
pub(super) fn paragraph_segments(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// Inserts `text` at the caret, turning every newline it carries into a
/// paragraph split.
///
/// Delegates straight to [`handle_character_key`] when there is no newline —
/// the overwhelmingly common path, which must stay allocation- and
/// behaviour-identical to before.
#[allow(clippy::too_many_arguments)] // mirrors `handle_character_key`'s arity
pub(super) fn insert_text_or_paragraph_breaks(
    text: String,
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    if !carries_newline(&text) {
        handle_character_key(
            text,
            focus,
            loro_doc,
            doc_state,
            cursor_state,
            undo_manager,
            can_undo,
            can_redo,
        );
        return;
    }

    let segments = paragraph_segments(&text);
    // Only the first operation may use the caller's `focus`: every step below
    // mutates the document and re-collapses the caret, so a stale position
    // would insert the rest of the payload at the wrong offset.
    let mut next = Some(focus);
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            let Some(pos) = next.take().or_else(|| cursor_state.read().focus.clone()) else {
                return;
            };
            handle_enter_key(
                pos,
                loro_doc,
                doc_state,
                cursor_state,
                undo_manager,
                can_undo,
                can_redo,
            );
        }
        if segment.is_empty() {
            continue;
        }
        let Some(pos) = next.take().or_else(|| cursor_state.read().focus.clone()) else {
            return;
        };
        handle_character_key(
            segment.clone(),
            pos,
            loro_doc,
            doc_state,
            cursor_state,
            undo_manager,
            can_undo,
            can_redo,
        );
    }
}

#[cfg(test)]
#[path = "editor_keydown_newline_tests.rs"]
mod tests;
