// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests against a real `loro::UndoManager`, locking the loro
//! behaviours the tracker relies on (fresh edits arrive with `Some(event)`,
//! undo/redo internal pushes with `None`).

use loro::{LoroDoc, UndoManager};

use super::SavedStateHandle;

fn doc_and_manager() -> (LoroDoc, UndoManager, SavedStateHandle) {
    let doc = LoroDoc::new();
    let mut um = UndoManager::new(&doc);
    let tracker = SavedStateHandle::new();
    tracker.attach(&mut um);
    (doc, um, tracker)
}

fn type_text(doc: &LoroDoc, s: &str) {
    let text = doc.get_text("t");
    let end = text.len_unicode();
    text.insert(end, s).unwrap();
    doc.commit();
}

#[test]
fn freshly_loaded_document_is_clean() {
    let (_doc, _um, tracker) = doc_and_manager();
    assert!(tracker.is_clean());
}

#[test]
fn an_edit_dirties_and_undo_restores_clean() {
    let (doc, mut um, tracker) = doc_and_manager();
    type_text(&doc, "a");
    assert!(!tracker.is_clean());
    um.undo().unwrap();
    assert!(tracker.is_clean(), "undo back to the loaded state is clean");
    um.redo().unwrap();
    assert!(!tracker.is_clean(), "redoing the edit is dirty again");
    um.undo().unwrap();
    assert!(tracker.is_clean());
}

#[test]
fn save_moves_the_clean_point() {
    let (doc, mut um, tracker) = doc_and_manager();
    type_text(&doc, "a");
    type_text(&doc, "b");
    tracker.mark_saved();
    assert!(tracker.is_clean());
    type_text(&doc, "c");
    assert!(!tracker.is_clean());
    um.undo().unwrap();
    assert!(tracker.is_clean(), "undo back to the save point is clean");
    um.undo().unwrap();
    assert!(!tracker.is_clean(), "undoing past the save point is dirty");
    um.redo().unwrap();
    assert!(
        tracker.is_clean(),
        "redo forward to the save point is clean"
    );
}

#[test]
fn saving_while_undone_makes_that_depth_the_clean_point() {
    let (doc, mut um, tracker) = doc_and_manager();
    type_text(&doc, "a");
    type_text(&doc, "b");
    um.undo().unwrap();
    tracker.mark_saved(); // save the "a" state with a live redo stack
    assert!(tracker.is_clean());
    um.redo().unwrap(); // "ab" differs from the file
    assert!(!tracker.is_clean());
    um.undo().unwrap();
    assert!(tracker.is_clean());
}

#[test]
fn editing_below_the_save_point_makes_it_unreachable() {
    let (doc, mut um, tracker) = doc_and_manager();
    type_text(&doc, "a");
    type_text(&doc, "b");
    tracker.mark_saved(); // clean at depth 2 ("ab")
    um.undo().unwrap(); // depth 1 ("a")
    type_text(&doc, "X"); // truncates the redo path back to "ab"
    assert!(
        !tracker.is_clean(),
        "\"aX\" at the saved depth is not the saved state"
    );
    // No amount of undo/redo can reach the saved state now.
    um.undo().unwrap();
    assert!(!tracker.is_clean());
    um.redo().unwrap();
    assert!(!tracker.is_clean());
    um.undo().unwrap();
    um.undo().unwrap();
    assert!(!tracker.is_clean());
    // Only a new save re-establishes a clean point.
    tracker.mark_saved();
    assert!(tracker.is_clean());
}

#[test]
fn a_replacement_manager_starts_clean_with_a_fresh_handle() {
    // Mirrors the post-save compaction swap: new doc, new manager, new
    // tracker — the save point is depth 0 of the fresh stack.
    let (doc, _um, _old_tracker) = doc_and_manager();
    type_text(&doc, "history");
    let fresh = LoroDoc::new();
    let mut um2 = UndoManager::new(&fresh);
    let tracker2 = SavedStateHandle::new();
    tracker2.attach(&mut um2);
    assert!(tracker2.is_clean());
    type_text(&fresh, "z");
    assert!(!tracker2.is_clean());
    um2.undo().unwrap();
    assert!(tracker2.is_clean());
}
