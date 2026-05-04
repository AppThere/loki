// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Per-page GPU paint source for `DocumentView`.
//!
//! [`LokiPageSource`] implements [`CustomPaintSource`] so that Blitz's frame
//! loop drives rendering.  On each frame it:
//!
//! 1. Reads the current [`CacheTier`] from the shared [`PageCache`].
//! 2. Reuses the registered [`TextureHandle`] when tier, document generation,
//!    and physical size are all unchanged (zero re-render cost).
//! 3. Otherwise unregisters the old texture, re-renders at the new tier's
//!    scale via Vello, and registers the fresh texture with Blitz.
//! 4. Updates the cache to record the new tier assignment.
//!
//! The texture lifetime follows the audit-documented pattern from
//! `loki-text/src/components/document_source.rs`: the handle is `None` until
//! the first render, unregistered before reallocation, and cleared (not
//! unregistered) in `suspend()` because the Vello renderer drop frees the
//! underlying allocation automatically.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use anyrender_vello::wgpu::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use loki_render_cache::{CacheTier, PageCache, PageIndex};
use loki_vello::FontDataCache;
use vello::{AaConfig, AaSupport, RenderParams, RendererOptions, Scene};

use crate::doc_page_source::DocPageSource;

// ── LokiPageSource ────────────────────────────────────────────────────────────

pub(crate) struct LokiPageSource {
    /// Shared tier-and-dirty metadata for all pages.
    cache: Arc<Mutex<PageCache>>,
    /// Document layout + page-size source.
    source: Arc<DocPageSource>,
    /// 0-based page index this source renders.
    page_index: usize,
    /// Lazily created Vello renderer (created in `resume()`).
    renderer: Option<vello::Renderer>,
    /// wgpu device from the last `resume()`.
    device: Option<anyrender_vello::wgpu::Device>,
    /// wgpu queue from the last `resume()`.
    wgpu_queue: Option<anyrender_vello::wgpu::Queue>,
    /// Font glyph cache — persisted across frames to avoid re-scanning fonts.
    font_cache: FontDataCache,
    /// Currently registered Blitz texture handle.
    /// Follows the audit lifecycle: None → registered → unregistered on change
    /// → cleared on suspend (renderer drop frees the allocation).
    texture_handle: Option<TextureHandle>,
    /// Tier at which `texture_handle` was rendered.
    texture_tier: Option<CacheTier>,
    /// Document generation at which `texture_handle` was rendered.
    texture_generation: u64,
    /// Physical pixel dimensions `(w, h)` of `texture_handle`.
    texture_size: (u32, u32),
}

impl LokiPageSource {
    pub(crate) fn new(
        cache: Arc<Mutex<PageCache>>,
        source: Arc<DocPageSource>,
        page_index: usize,
    ) -> Self {
        Self {
            cache,
            source,
            page_index,
            renderer: None,
            device: None,
            wgpu_queue: None,
            font_cache: FontDataCache::new(),
            texture_handle: None,
            texture_tier: None,
            texture_generation: 0,
            texture_size: (0, 0),
        }
    }
}

// ── CustomPaintSource ─────────────────────────────────────────────────────────

impl CustomPaintSource for LokiPageSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.device = Some(device_handle.device.clone());
        self.wgpu_queue = Some(device_handle.queue.clone());

        match vello::Renderer::new(
            &device_handle.device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        ) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => tracing::warn!(
                page = self.page_index,
                error = %e,
                "LokiPageSource: vello renderer init failed",
            ),
        }
    }

    fn suspend(&mut self) {
        // Vello renderer drop frees image_overrides (registered textures).
        // Clear stale handle so the next render() after resume() doesn't
        // attempt to reuse a handle from the dead renderer.
        self.renderer = None;
        self.device = None;
        self.wgpu_queue = None;
        self.texture_handle = None;
        self.texture_tier = None;
        self.texture_generation = 0;
        self.texture_size = (0, 0);
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        let (Some(device), Some(queue), Some(renderer)) =
            (self.device.as_ref(), self.wgpu_queue.as_ref(), self.renderer.as_mut())
        else {
            return None;
        };

        // Step 1: read current tier from cache (default Hot for first frame).
        let current_tier = self
            .cache
            .lock()
            .ok()
            .and_then(|g| g.get(PageIndex(self.page_index as u32)).map(|p| p.tier))
            .unwrap_or(CacheTier::Hot);

        // Step 2: compute target physical texture dimensions.
        // width/height from Blitz are already in physical (device) pixels.
        let scale_factor = current_tier.scale_factor();
        let w_phys = ((width as f32 * scale_factor).ceil() as u32).max(1);
        let h_phys = ((height as f32 * scale_factor).ceil() as u32).max(1);

        // Step 3: read current document generation for reuse guard and layout.
        let current_generation = self.source.current_generation();

        // Step 4: reuse guard — return existing handle when nothing changed.
        if self.texture_handle.is_some()
            && self.texture_tier == Some(current_tier)
            && self.texture_generation == current_generation
            && self.texture_size == (w_phys, h_phys)
        {
            return self.texture_handle.clone();
        }

        // Step 5: unregister stale texture before reallocating.
        if let Some(old) = self.texture_handle.take() {
            ctx.unregister_texture(old);
        }

        // Step 6: allocate new GPU texture.
        // COMPAT(blitz): Rgba8Unorm + STORAGE_BINDING|TEXTURE_BINDING matches
        // the format expected by anyrender_vello register_texture.
        let texture = device.create_texture(&anyrender_vello::wgpu::TextureDescriptor {
            label: Some("loki-page"),
            size: Extent3d { width: w_phys, height: h_phys, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());

        // Step 7: build Vello scene for this page.
        // loki-layout uses points (1/72 in); Blitz's `scale` is DPR.
        // render_scale maps points → physical pixels at the tier's resolution:
        //   DPR × tier_scale_factor × (96 CSS-px / 72 pt)
        // This ensures the scene exactly fills w_phys × h_phys.
        let layout_guard = self.source.layout_for_generation(current_generation);
        let (_, layout) = layout_guard.as_ref()?;
        let mut scene = Scene::new();
        let render_scale = scale as f32 * scale_factor * (96.0 / 72.0);
        loki_vello::paint_single_page(
            &mut scene,
            layout,
            &mut self.font_cache,
            (0.0, 0.0),
            render_scale,
            self.page_index,
            None,
        );
        drop(layout_guard);

        // Step 8: render scene to texture.
        let params = RenderParams {
            base_color: vello::peniko::Color::WHITE,
            width: w_phys,
            height: h_phys,
            antialiasing_method: AaConfig::Msaa16,
        };
        if let Err(e) = renderer.render_to_texture(device, queue, &scene, &view, &params) {
            tracing::error!(
                page = self.page_index,
                tier = ?current_tier,
                error = %e,
                "LokiPageSource: render_to_texture failed",
            );
            return None;
        }

        // Step 9: register with Blitz and cache the handle.
        let handle = ctx.register_texture(texture);
        self.texture_handle = Some(handle.clone());
        self.texture_tier = Some(current_tier);
        self.texture_generation = current_generation;
        self.texture_size = (w_phys, h_phys);

        // Step 10: update cache metadata with the rendered tier.
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(PageIndex(self.page_index as u32), current_tier);
        }

        tracing::debug!(
            page  = self.page_index,
            tier  = ?current_tier,
            w     = w_phys,
            h     = h_phys,
            "LokiPageSource: rendered",
        );

        Some(handle)
    }
}
