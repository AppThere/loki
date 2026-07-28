// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the residency model. Extracted per the file-ceiling idiom.

use super::{
    PageBox, ViewportSpec, resident_pages, resident_texture_bytes, texture_bytes, visible_window,
};

const H: f64 = 1000.0; // page height, CSS px
const GAP: f64 = 20.0;

/// A document of `n` identical A4 pages.
fn a4_doc(n: usize) -> Vec<PageBox> {
    vec![PageBox::a4(); n]
}

// ── The mounting rule (moved from loki_renderer::virtualize) ─────────────────

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

// ── Texture arithmetic ───────────────────────────────────────────────────────

#[test]
fn a_page_texture_is_four_bytes_per_device_pixel() {
    assert_eq!(texture_bytes(100, 200), 100 * 200 * 4);
}

#[test]
fn us_letter_matches_the_s0_2_arithmetic() {
    // S0.2 §3: 612 × 792 pt -> 816 × 1056 CSS px -> 3_446_784 bytes at z·s = 1.
    let letter = PageBox::us_letter();
    assert_eq!(letter.texture_size_px(1.0, 1.0), (816, 1056));
    assert_eq!(letter.texture_bytes(1.0, 1.0), 3_446_784);
    // ...and its quoted worst case at 200% on a HiDPI display: z·s = 4, so 16×.
    // S0.2 prints this as "55.15 MB", which is decimal MB — 52.6 MiB. Asserted
    // in bytes so the unit cannot drift silently between the spike and here.
    assert_eq!(letter.texture_bytes(2.0, 2.0), 55_148_544);
}

#[test]
fn zoom_and_device_scale_are_interchangeable_in_the_texture_cost() {
    // The claim §3.2 rests on: the axis is the *product*, so 100% on a HiDPI
    // display costs exactly what 200% on a standard display costs.
    let page = PageBox::us_letter();
    assert_eq!(page.texture_bytes(1.0, 2.0), page.texture_bytes(2.0, 1.0));
}

#[test]
fn degenerate_scales_clamp_to_one_pixel_rather_than_exploding() {
    let page = PageBox::a4();
    assert_eq!(page.texture_size_px(0.0, 1.0), (1, 1));
    // Non-finite is a bug upstream; it collapses to the 1 px floor rather than
    // propagating as a nonsense allocation size.
    assert_eq!(page.texture_size_px(f64::NAN, 1.0), (1, 1));
    assert_eq!(page.texture_size_px(f64::INFINITY, 1.0), (1, 1));
    // A finite but absurd zoom saturates at the u32 ceiling instead of
    // wrapping, so the byte figure stays monotonic in zoom.
    assert_eq!(page.texture_size_px(1e12, 1.0), (u32::MAX, u32::MAX));
}

// ── Residency ────────────────────────────────────────────────────────────────

#[test]
fn resident_bytes_are_flat_in_document_length() {
    // §3.2, the whole reason Phase 2's scope is zoom × DPI: once a document
    // exceeds the window, adding pages adds no resident texture bytes. The r1
    // acceptance criterion (500-page RSS within 20% of 10-page) was withdrawn
    // because texture work cannot move this number — it is already equal.
    let vp = ViewportSpec::new(20_000.0, 900.0, 1.0, 1.0);
    let hundred = resident_texture_bytes(&a4_doc(100), &vp);
    let five_hundred = resident_texture_bytes(&a4_doc(500), &vp);
    assert_eq!(hundred, five_hundred);
    assert!(hundred > 0, "the window must hold something at this offset");
}

#[test]
fn resident_bytes_grow_with_zoom_and_device_scale() {
    let doc = a4_doc(200);
    let at_100 = resident_texture_bytes(&doc, &ViewportSpec::new(20_000.0, 900.0, 1.0, 1.0));
    let at_200 = resident_texture_bytes(&doc, &ViewportSpec::new(20_000.0, 900.0, 2.0, 1.0));
    let hidpi_200 = resident_texture_bytes(&doc, &ViewportSpec::new(20_000.0, 900.0, 2.0, 2.0));
    assert!(at_200 > at_100, "zoom must cost more, not less");
    assert!(hidpi_200 > at_200, "HiDPI must cost more than standard DPI");
    // Per page the cost is quadratic in zoom, but taller pages mean fewer of
    // them mount, so the *total* grows sub-quadratically. Pinned as a range
    // rather than a value because the page count is a step function.
    let ratio = hidpi_200 as f64 / at_100 as f64;
    assert!(
        (2.0..16.0).contains(&ratio),
        "4x per-page cost, fewer pages resident: expected 2x..16x, got {ratio:.2}x"
    );
}

#[test]
fn only_windowed_pages_are_counted() {
    let doc = a4_doc(200);
    let vp = ViewportSpec::new(20_000.0, 900.0, 1.0, 1.0);
    let resident = resident_pages(&doc, &vp);
    let count = resident.iter().filter(|&&v| v).count();
    assert_eq!(
        resident_texture_bytes(&doc, &vp),
        count as u64 * PageBox::a4().texture_bytes(1.0, 1.0),
    );
    assert!(
        count < doc.len(),
        "a 200-page document must not fully mount"
    );
}

#[test]
fn an_empty_document_is_free() {
    let vp = ViewportSpec::new(0.0, 900.0, 1.0, 1.0);
    assert_eq!(resident_texture_bytes(&[], &vp), 0);
}

/// **Which mechanism drops a page on scroll-back: the mount window, or the
/// budget?** Settled with the model rather than by arithmetic on a log.
///
/// A screen session on macOS at 100% zoom on a 2× display showed pages 11, 10,
/// 9 and 8 re-rasterising while scrolling back up from page 13, with the budget
/// reporting `source=Baseline` (64 MiB). The reading offered was eviction under
/// budget pressure — 64 MiB holds 4.87 of these 13.15 MiB pages — which would
/// make the dead memory probe a live performance cost.
///
/// It is not what happened, and this table is why. The mount window is the
/// visible rect **grown by one screen on each side** (`visible_window`), so on a
/// laptop-sized viewport it asks for three or four pages — 39–53 MiB, inside the
/// baseline with room to spare. Pages four pitches away were never resident, at
/// any budget.
///
/// **The log carries its own confirmation:** `tile_plan` emits "texture
/// residency under pressure" whenever `over_target || reduced_tiles > 0`, and no
/// such line appeared. The budget never bound.
///
/// Where the baseline *does* bind is a large HiDPI display: from roughly
/// 1100–1200 CSS px of viewport the window reaches five pages. That is a real
/// finding and a different one — a forecast about big screens, not an
/// observation about this session.
///
/// The consequence for what to fix: if scroll-back re-rasterisation is judged
/// too costly, the lever is **retention hysteresis** — a wider window, or an
/// eviction policy with memory — and not a larger budget. Raising the budget
/// would change nothing on the machine that produced the log.
#[test]
fn the_mount_window_not_the_budget_decides_scroll_back_residency_at_100_percent() {
    // US Letter at 96 dpi on a 2x display: 1632 x 2112 device px = 13.15 MiB.
    let page = PageBox::us_letter();
    let pages: Vec<PageBox> = (0..24).map(|_| page).collect();
    let per_page = page.texture_bytes(1.0, 2.0);
    assert_eq!(
        per_page, 13_787_136,
        "fixture must be the page the log reported, or the table below is about \
         a different machine",
    );
    let baseline = 64 * 1024 * 1024;

    for (client_h, expect_binds) in [
        (700.0_f64, false),
        (835.0, false), // ~13" laptop, chrome subtracted — the session's shape.
        (1000.0, false),
        (1200.0, true), // A large HiDPI display: five pages in the window.
        (1600.0, true),
    ] {
        // Scrolled well into the document, so the window is interior and the
        // count is the steady-state one rather than a document-edge case.
        let vp = ViewportSpec::new(6.0 * (page.css_size(1.0).1 + 24.0), client_h, 1.0, 2.0);
        let bytes = resident_texture_bytes(&pages, &vp);
        let resident = resident_pages(&pages, &vp).iter().filter(|r| **r).count();
        assert_eq!(
            bytes > baseline,
            expect_binds,
            "at a {client_h}px viewport the window holds {resident} pages \
             ({:.2} MiB) against a {:.0} MiB baseline",
            bytes as f64 / 1_048_576.0,
            baseline as f64 / 1_048_576.0,
        );
    }
}
