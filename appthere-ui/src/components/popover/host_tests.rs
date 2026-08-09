// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The root-layer ordering, asserted rather than left to `rsx!` order.

use super::RootLayer;

/// **The failure this ordering prevents:** the backdrop paints over the popup,
/// so the menu is visible and every click on it is swallowed. That reads as a
/// dead menu, and nobody looking at a dead menu suspects paint order.
#[test]
fn the_backdrop_mounts_below_the_overlay() {
    assert!(
        RootLayer::Backdrop.mount_order() < RootLayer::Overlay.mount_order(),
        "the backdrop must be an earlier child of the root than the overlay, or \
         it paints over the popup it exists to serve",
    );
    assert!(RootLayer::Backdrop < RootLayer::Overlay);
}

/// The ordering is total and distinct — two layers sharing a position would make
/// the paint order depend on `rsx!` again, which is what this replaces.
#[test]
fn each_layer_has_its_own_position() {
    assert_ne!(
        RootLayer::Backdrop.mount_order(),
        RootLayer::Overlay.mount_order(),
    );
}
