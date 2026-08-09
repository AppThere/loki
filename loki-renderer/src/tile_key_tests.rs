// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Invalidation tests — Spec 08 T2.3 and risk R10. Extracted per the
//! file-ceiling idiom.
//!
//! Each test drives the *real* tile arithmetic
//! (`appthere_canvas::residency::PageBox::texture_size_px`) rather than picking
//! two different numbers by hand. Asserting that different sizes give different
//! keys would prove nothing about zoom; what R10 wants to know is that changing
//! the zoom changes the size, and therefore the key.

use super::TileKey;
use crate::document_view::{RendererCursorPos, RendererSelection};
use appthere_canvas::residency::PageBox;

fn key_at(zoom: f64, dsf: f64) -> TileKey {
    TileKey::new(
        7,
        PageBox::us_letter().texture_size_px(zoom, dsf),
        None,
        1.0,
    )
}

fn caret(byte_offset: usize) -> Option<RendererSelection> {
    let pos = RendererCursorPos {
        page_index: 0,
        paragraph_index: 3,
        byte_offset,
    };
    Some(RendererSelection {
        focus: pos,
        anchor: pos,
    })
}

#[test]
fn changing_the_zoom_invalidates_the_tile() {
    // R10, the explicit zoom-invalidation test. Zoom rework and texture
    // invalidation interact, and a stale tile after a zoom change is not a
    // subtle failure — it is the page rendered at the wrong size.
    let base = key_at(1.0, 1.0);
    for zoom in [0.25, 0.5, 1.25, 2.0, 4.0] {
        assert_ne!(base, key_at(zoom, 1.0), "zoom {zoom} must invalidate");
    }
    assert_eq!(base, key_at(1.0, 1.0), "an unchanged zoom must reuse");
}

#[test]
fn changing_the_device_scale_factor_invalidates_the_tile() {
    // The other half of the zoom x DPI axis. A window dragged to a HiDPI
    // display must re-rasterise, not upscale the standard-DPI texture.
    let base = key_at(1.0, 1.0);
    for dsf in [1.5, 2.0, 3.0] {
        assert_ne!(base, key_at(1.0, dsf), "dsf {dsf} must invalidate");
    }
}

#[test]
fn zoom_and_device_scale_are_not_independently_tracked_and_should_not_be() {
    // 200% at 1x and 100% at 2x produce the same pixels. The texture is
    // identical, so reusing it is correct — the key is over what a texture is,
    // not over what produced it. Pinned so a future "track zoom separately"
    // change has to argue with a test instead of looking like a tightening.
    assert_eq!(key_at(2.0, 1.0), key_at(1.0, 2.0));
}

#[test]
fn an_edit_invalidates_the_tile() {
    let size = PageBox::us_letter().texture_size_px(1.0, 1.0);
    assert_ne!(
        TileKey::new(7, size, None, 1.0),
        TileKey::new(8, size, None, 1.0),
    );
}

#[test]
fn a_page_style_change_that_moves_the_paper_size_invalidates_the_tile() {
    // Same generation would already invalidate via the edit path, but the size
    // must move too — otherwise a Letter/A4 switch at identical generation
    // (a re-layout with no mutation) would reuse a wrongly-shaped texture.
    let letter = PageBox::us_letter().texture_size_px(1.0, 1.0);
    let a4 = PageBox::a4().texture_size_px(1.0, 1.0);
    assert_ne!(letter, a4);
    assert_ne!(
        TileKey::new(7, letter, None, 1.0),
        TileKey::new(7, a4, None, 1.0),
    );
}

#[test]
fn caret_movement_invalidates_the_tile() {
    let size = PageBox::us_letter().texture_size_px(1.0, 1.0);
    assert_ne!(
        TileKey::new(7, size, caret(10), 1.0),
        TileKey::new(7, size, caret(11), 1.0),
    );
    assert_eq!(
        TileKey::new(7, size, caret(10), 1.0),
        TileKey::new(7, size, caret(10), 1.0),
    );
}

#[test]
fn a_rasterisation_scale_change_invalidates_the_tile() {
    // T2.2's lever has to be part of the key, or a tile reduced under budget
    // pressure would keep its full-scale texture and save nothing.
    let size = PageBox::us_letter().texture_size_px(2.0, 2.0);
    assert_ne!(
        TileKey::new(7, size, None, 1.0),
        TileKey::new(7, size, None, 0.5),
    );
}

#[test]
fn scale_quantisation_absorbs_float_noise_but_not_a_real_step() {
    // A scale arriving from a square root differs in the last mantissa bit
    // between frames. Without quantisation that would re-render every tile,
    // every frame, and the budget would cost more than it saved.
    let size = PageBox::us_letter().texture_size_px(2.0, 2.0);
    let a = TileKey::new(7, size, None, 0.500_000_1);
    let b = TileKey::new(7, size, None, 0.499_999_9);
    assert_eq!(a, b, "sub-thousandth noise must not invalidate");
    assert_ne!(a, TileKey::new(7, size, None, 0.499));
}

#[test]
fn a_degenerate_scale_is_clamped_rather_than_producing_a_zero_texture() {
    assert_eq!(super::quantise(0.0), 1);
    assert_eq!(super::quantise(-1.0), 1);
    assert_eq!(super::quantise(f32::NAN), 1000);
    assert_eq!(super::quantise(2.0), 1000);
    assert_eq!(super::quantise(1.0), 1000);
}
