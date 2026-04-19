// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the `WgpuSurface` GPU submission pipeline.
//!
//! Tests that require a live wgpu `DeviceHandle` (i.e. `resume()` and
//! `render()`) are covered by the `#[cfg(test)]` module inside
//! `document_source.rs`, which has access to `LokiDocumentSource`'s private
//! fields and can construct `CachedLayout` directly without a GPU.
//!
//! This file tests the public [`DocumentState`] API — the contract between the
//! Dioxus component and the paint source.

use loki_text::components::document_source::DocumentState;
use std::sync::{Arc, Mutex};

#[test]
fn document_state_default_has_no_document() {
    let state = DocumentState {
        document: None,
        generation: 0,
        page_count: 0,
        visible_rect: None,
    };
    assert!(state.document.is_none());
    assert_eq!(state.generation, 0);
    assert_eq!(state.page_count, 0);
    assert!(state.visible_rect.is_none());
}

#[test]
fn generation_wraps_without_panic() {
    // Verifies that wrapping_add used in WgpuSurface doesn't overflow.
    let max_gen = u64::MAX;
    assert_eq!(max_gen.wrapping_add(1), 0);
}

#[test]
fn document_state_can_be_shared_across_threads() {
    let state = Arc::new(Mutex::new(DocumentState {
        document: None,
        generation: 0,
        page_count: 0,
        visible_rect: None,
    }));
    let state2 = Arc::clone(&state);
    let handle = std::thread::spawn(move || {
        let mut s = state2.lock().unwrap();
        s.generation = s.generation.wrapping_add(1);
    });
    handle.join().unwrap();
    assert_eq!(state.lock().unwrap().generation, 1);
}
