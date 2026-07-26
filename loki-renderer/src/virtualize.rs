// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Viewport virtualization for page/band tiles.
//!
//! Opening a document GPU-paints one tile per page on the first composite. For a
//! long document that is the dominant open-latency cost (and a large amount of
//! texture memory). [`visible_window`] restricts the GPU tiles to the pages near
//! the viewport; the rest render as cheap page-sized placeholders and become
//! real tiles when scrolled near.
//!
//! # The rule itself lives in `appthere-canvas`
//!
//! Spec 08 Phase 2 needs to sweep resident texture bytes across zoom × DPI
//! headlessly, which means the mounting rule has to be reachable without
//! Dioxus, Blitz or wgpu. It moved to `appthere_canvas::residency`, and this
//! module re-exports it: production and the Phase 2 model run **the same
//! function**, so they cannot disagree. A model that restated the rule would
//! drift from it, and a drifted model measures itself.

pub(crate) use appthere_canvas::residency::visible_window;

#[cfg(test)]
mod tests {
    use super::visible_window;

    /// The window rule's own tests live beside it in
    /// `appthere-canvas/src/residency/geometry_tests.rs`. What is left to check
    /// here is that this crate is wired to that rule at all — a re-export that
    /// silently pointed at a local copy would pass every test in both crates.
    #[test]
    fn virtualization_is_wired_to_the_shared_residency_rule() {
        let heights = vec![1000.0_f64; 20];
        let vis = visible_window(&heights, 20.0, 0.0, 900.0);
        assert_eq!(vis.len(), 20);
        assert!(vis[0] && vis[1], "the viewport neighbourhood mounts");
        assert!(
            vis[2..].iter().all(|&v| !v),
            "distant pages stay virtualized"
        );
    }
}
