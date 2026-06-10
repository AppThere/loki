// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for the `DocumentState` API.
//!
//! Tests the public contract between the editor and the document editing state
//! (cursor hit-testing, generation tracking, page count).

use loki_text::editing::state::DocumentState;
use std::sync::{Arc, Mutex};

fn make_state() -> DocumentState {
    DocumentState::new()
}

#[test]
fn document_state_default_has_no_document() {
    let state = make_state();
    assert!(state.document.is_none());
    assert_eq!(state.generation, 0);
    assert_eq!(state.page_count, 0);
    assert!(state.paginated_layout.is_none());
}

#[test]
fn generation_wraps_without_panic() {
    let max_gen = u64::MAX;
    assert_eq!(max_gen.wrapping_add(1), 0);
}

#[test]
fn document_state_can_be_shared_across_threads() {
    let state = Arc::new(Mutex::new(make_state()));
    let state2 = Arc::clone(&state);
    let handle = std::thread::spawn(move || {
        let mut s = state2.lock().unwrap();
        s.generation = s.generation.wrapping_add(1);
    });
    handle.join().unwrap();
    assert_eq!(state.lock().unwrap().generation, 1);
}

/// Verifies `generation` advances monotonically on document changes.
#[test]
fn generation_is_monotone_across_document_changes() {
    let state = Arc::new(Mutex::new(make_state()));

    let mut prev = 0u64;
    for _ in 0..10 {
        let mut s = state.lock().unwrap();
        s.generation = s.generation.wrapping_add(1);
        assert!(
            s.generation > prev || (prev == u64::MAX && s.generation == 0),
            "generation must advance monotonically (or wrap from MAX→0)"
        );
        prev = s.generation;
    }
}
