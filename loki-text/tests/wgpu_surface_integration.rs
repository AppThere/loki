// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the `WgpuSurface` GPU submission pipeline.
//!
//! # Texture-handle leak prevention
//!
//! The confirmed memory-leak site was `LokiDocumentSource::render()` calling
//! `ctx.register_texture()` every frame without a corresponding
//! `ctx.unregister_texture()`.  `vello::Renderer::register_texture` takes
//! ownership of the `wgpu::Texture` by value and inserts it into an internal
//! `image_overrides` HashMap; without `unregister_texture` the map grows by
//! one entry per frame, leaking GPU memory continuously.
//!
//! The fix stores a `TextureHandle` across frames, calls `unregister_texture`
//! on the previous handle before allocating a new texture, and reuses the
//! existing handle when the document generation and physical canvas size have
//! not changed.
//!
//! Per-frame structural tests (handle init, reuse guard, suspend cleanup) live
//! in the `#[cfg(test)]` module inside `document_source.rs` because
//! `LokiDocumentSource` is `pub(crate)`.  Full render-loop leak detection
//! (calling `render()` 10× on a headless device and asserting one live
//! allocation) requires a wgpu device; see the unit test module for the
//! structural invariants that guarantee the same property without GPU.
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
        canvas_width: 0.0,
        visible_rect: None,
        page_width_px: 0.0,
        page_height_px: 0.0,
        cursor_state: None,
        preserve_for_editing: false,
        paginated_layout: None,
    };
    assert!(state.document.is_none());
    assert_eq!(state.generation, 0);
    assert_eq!(state.page_count, 0);
    assert_eq!(state.canvas_width, 0.0);
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
        canvas_width: 0.0,
        visible_rect: None,
        page_width_px: 0.0,
        page_height_px: 0.0,
        cursor_state: None,
        preserve_for_editing: false,
        paginated_layout: None,
    }));
    let state2 = Arc::clone(&state);
    let handle = std::thread::spawn(move || {
        let mut s = state2.lock().unwrap();
        s.generation = s.generation.wrapping_add(1);
    });
    handle.join().unwrap();
    assert_eq!(state.lock().unwrap().generation, 1);
}

/// Simulates the WgpuSurface component bumping `generation` on N consecutive
/// document changes and verifies the counter is monotonically non-decreasing.
/// This is the signal `LokiDocumentSource` uses to decide whether to discard
/// its cached texture handle and render a new texture; stale counters would
/// cause the reuse guard to suppress re-renders after a document change.
#[test]
fn generation_is_monotone_across_document_changes() {
    let state = Arc::new(Mutex::new(DocumentState {
        document: None,
        generation: 0,
        page_count: 0,
        canvas_width: 0.0,
        visible_rect: None,
        page_width_px: 0.0,
        page_height_px: 0.0,
        cursor_state: None,
        preserve_for_editing: false,
        paginated_layout: None,
    }));

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
