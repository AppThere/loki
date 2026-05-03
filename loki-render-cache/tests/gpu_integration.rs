// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests that require a real wgpu device.
//!
//! Gated on the `gpu` feature. Run with:
//! ```text
//! cargo test -p loki-render-cache --no-default-features --features gpu
//! ```

#![cfg(feature = "gpu")]

use std::sync::{Arc, Mutex};

use loki_render_cache::{
    CacheTier, PageCache, PageGeometry, PageIndex, PageSource, RenderError, RenderQueue,
    ScrollState, allocate_texture,
    texture::GpuTexture,
};

// ── Minimal PageSource stub ───────────────────────────────────────────────────

struct SolidPageSource {
    /// All test pages are this size at 1× scale.
    width: u32,
    height: u32,
    /// Total number of pages available.
    page_count: u32,
}

impl PageSource for SolidPageSource {
    fn page_size_px(&self, _index: PageIndex) -> (u32, u32) {
        (self.width, self.height)
    }

    fn render(
        &self,
        index: PageIndex,
        scale: f32,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<GpuTexture, RenderError> {
        if index.0 >= self.page_count {
            return Err(RenderError::NoSuchPage(index));
        }
        let w = ((self.width as f32 * scale).ceil() as u32).max(1);
        let h = ((self.height as f32 * scale).ceil() as u32).max(1);
        // Allocate a blank texture; no pixel data needed for cache tests.
        Ok(allocate_texture(device, w, h, Some("test-page")))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Try to obtain a wgpu adapter + device. Returns `None` when no GPU or
/// software rasteriser is available (CI without GPU support).
fn try_wgpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: None,
        force_fallback_adapter: true,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()
}

/// Build a ScrollState with the viewport covering both test pages.
fn scroll_covering_pages() -> ScrollState {
    // Pages 0..2 each 200 px tall at y = 0..400.
    // hot zone for viewport_top=0, height=800: (-400, 1200) → both pages Hot.
    let s = ScrollState::new(800.0);
    s
}

fn page_geom(index: u32) -> PageGeometry {
    let top = f64::from(index) * 210.0;
    PageGeometry { index, top_px: top, bottom_px: top + 200.0 }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn retier_and_render_queue_populates_cache() {
    let Some((device, queue)) = try_wgpu() else {
        eprintln!("gpu_integration: no wgpu adapter — skipping");
        return;
    };

    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let source: Arc<dyn PageSource> = Arc::new(SolidPageSource {
        width: 100,
        height: 200,
        page_count: 2,
    });

    let cache = Arc::new(Mutex::new(PageCache::new(u64::MAX)));
    let render_queue = RenderQueue::new(
        Arc::clone(&cache),
        Arc::clone(&source),
        Arc::clone(&device),
        Arc::clone(&queue),
    );

    // Both pages are uncached → retier should list them in `rerender`.
    let pages = [page_geom(0), page_geom(1)];
    let scroll = scroll_covering_pages();
    let result = {
        let mut c = cache.lock().unwrap();
        c.retier(&pages, &scroll)
    };

    assert_eq!(result.rerender.len(), 2, "both pages should need re-render");
    assert!(result.downsample.is_empty());
    assert!(result.evicted.is_empty());

    // Submit jobs; the worker renders and inserts into the cache.
    render_queue.submit(result);
    render_queue.shutdown();

    // After shutdown the worker thread has finished all jobs.
    let c = cache.lock().unwrap();
    for i in 0..2u32 {
        let page = c.get(PageIndex(i)).expect("page should be in cache after render");
        assert!(!page.dirty, "page {i} should be clean after insert");
        assert_eq!(page.tier, CacheTier::Hot, "page {i} should be Hot");
    }
}
