// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests that require a real wgpu device.
//!
//! Gated on the `gpu` feature. Run with:
//! ```text
//! cargo test -p loki-render-cache --no-default-features --features gpu
//! ```

#![cfg(feature = "gpu")]

use loki_render_cache::{
    CacheTier, PageCache, PageGeometry, PageIndex, ScrollState, allocate_texture,
    texture::downsample_texture,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Try to obtain a wgpu adapter + device. Returns `None` when no GPU or
/// software rasteriser is available (CI without GPU support).
fn try_wgpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()
}

fn page_geom(index: u32) -> PageGeometry {
    let top = f64::from(index) * 210.0;
    PageGeometry { index, top_px: top, bottom_px: top + 200.0 }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn retier_marks_pages_for_rerender_and_cache_is_updated() {
    // Tier-policy logic runs without GPU; this test verifies the metadata path.
    let mut cache = PageCache::new();

    let pages = [page_geom(0), page_geom(1)];
    let scroll = ScrollState::new(800.0);
    let result = cache.retier(&pages, &scroll);

    // Both pages are uncached → retier should list them in `rerender`.
    assert_eq!(result.rerender.len(), 2, "both pages should need re-render");
    assert!(result.downsample.is_empty());

    // Simulate LokiPageSource inserting after render.
    for (idx, tier) in &result.rerender {
        cache.insert(*idx, *tier);
    }

    for i in 0..2u32 {
        let page = cache.get(PageIndex(i)).expect("page should be in cache after insert");
        assert!(!page.dirty, "page {i} should be clean after insert");
        assert_eq!(page.tier, CacheTier::Hot, "page {i} should be Hot");
    }
}

#[test]
fn allocate_and_downsample_texture_succeeds() {
    let Some((device, queue)) = try_wgpu() else {
        eprintln!("gpu_integration: no wgpu adapter — skipping");
        return;
    };

    let full = allocate_texture(&device, 100, 200, Some("test-full"));
    assert_eq!(full.width, 100);
    assert_eq!(full.height, 200);

    let half = downsample_texture(&device, &queue, &full, 0.5);
    assert_eq!(half.width, 50);
    assert_eq!(half.height, 100);
}
