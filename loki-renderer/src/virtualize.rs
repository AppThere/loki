// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Viewport virtualization for page/band tiles.
//!
//! Opening a document GPU-paints one tile per page on the first composite. For a
//! long document that is the dominant open-latency cost (and a large amount of
//! texture memory). [`visible_window`] restricts the GPU tiles to the pages near
//! the viewport; the rest render as cheap page-sized placeholders and become
//! real tiles when scrolled near.

/// Returns, per page in document order, whether it falls within the tile
/// virtualization window: the visible range `[viewport_top, viewport_top + vh]`
/// grown by one screen (`vh`) on each side.
///
/// `page_heights` are CSS-px heights in document order, separated by `gap_px`.
/// A document shorter than the window has every page visible, so short documents
/// transparently behave as before (render everything).
pub(crate) fn visible_window(
    page_heights: &[f64],
    gap_px: f64,
    viewport_top: f64,
    viewport_height: f64,
) -> Vec<bool> {
    let vh = viewport_height.max(1.0);
    let win_lo = viewport_top - vh;
    let win_hi = viewport_top + 2.0 * vh;
    let mut page_top = 0.0_f64;
    page_heights
        .iter()
        .map(|&h| {
            // Overlap test between [page_top, page_top + h] and [win_lo, win_hi].
            let visible = (page_top + h) >= win_lo && page_top <= win_hi;
            page_top += h + gap_px;
            visible
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::visible_window;

    const H: f64 = 1000.0; // page height
    const GAP: f64 = 20.0;

    #[test]
    fn document_within_the_window_renders_every_page() {
        // A document that fits inside the window (here, two short pages) has every
        // page visible, so short documents behave exactly as before.
        let vis = visible_window(&[500.0, 500.0], GAP, 0.0, 900.0);
        assert_eq!(vis, vec![true, true]);
    }

    #[test]
    fn long_document_at_top_only_renders_the_neighbourhood() {
        let heights = vec![H; 20];
        let vis = visible_window(&heights, GAP, 0.0, 900.0);
        // Window is [-900, 1800]; pages 0 (0..1000) and 1 (1020..2020) overlap;
        // page 2 (2040..) does not.
        assert!(vis[0] && vis[1]);
        assert!(!vis[2], "far pages must be virtualized");
        assert!(vis[2..].iter().all(|&v| !v));
    }

    #[test]
    fn window_follows_the_viewport() {
        let heights = vec![H; 20];
        // Scrolled so the viewport top sits on page 10 (top = 10*(H+GAP)).
        let top = 10.0 * (H + GAP);
        let vis = visible_window(&heights, GAP, top, 900.0);
        assert!(vis[10], "the page under the viewport is visible");
        assert!(!vis[0] && !vis[19], "distant pages are virtualized");
        // Margins reach roughly one screen each side, not the whole document.
        let visible_count = vis.iter().filter(|&&v| v).count();
        assert!(
            (2..=6).contains(&visible_count),
            "window should be a small neighbourhood, got {visible_count}"
        );
    }

    #[test]
    fn zero_viewport_height_is_safe() {
        // Degenerate height must not panic or render nothing pathologically.
        let vis = visible_window(&[H, H], GAP, 0.0, 0.0);
        assert_eq!(vis.len(), 2);
        assert!(vis[0], "the page at the viewport top stays visible");
    }
}
