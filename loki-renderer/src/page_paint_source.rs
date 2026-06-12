// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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

use std::sync::{Arc, Mutex};

use anyrender_vello::wgpu::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use appthere_canvas::{CacheTier, PageCache, PageIndex};
use loki_vello::FontDataCache;
use vello::{AaConfig, RenderParams, Scene};

use crate::doc_page_source::DocPageSource;
use crate::document_view::RendererCursorPos;

// ── LokiPageSource ────────────────────────────────────────────────────────────

pub(crate) struct LokiPageSource {
    /// Shared tier-and-dirty metadata for all pages.
    cache: Arc<Mutex<PageCache<PageIndex>>>,
    /// Document layout + page-size source.
    source: Arc<DocPageSource>,
    /// 0-based page index this source renders.
    page_index: usize,
    /// Shared Vello renderer — created by the first page source to resume.
    ///
    // COMPAT(loki): first page source to resume creates the shared renderer.
    // Subsequent page sources find it populated and skip creation.
    renderer: Arc<Mutex<Option<vello::Renderer>>>,
    /// wgpu device from the last `resume()`.
    device: Option<anyrender_vello::wgpu::Device>,
    /// wgpu queue from the last `resume()`.
    wgpu_queue: Option<anyrender_vello::wgpu::Queue>,
    /// Font glyph cache — persisted across frames to avoid re-scanning fonts.
    font_cache: FontDataCache,
    /// Currently registered Blitz texture handle.
    texture_handle: Option<TextureHandle>,
    /// Tier at which `texture_handle` was rendered.
    texture_tier: Option<CacheTier>,
    /// Document generation at which `texture_handle` was rendered.
    texture_generation: u64,
    /// Physical pixel dimensions `(w, h)` of `texture_handle`.
    texture_size: (u32, u32),
    /// Shared cursor position written by PageTile on every Dioxus render.
    cursor_holder: Arc<Mutex<Option<RendererCursorPos>>>,
    /// Cursor position at which `texture_handle` was rendered — used to
    /// invalidate the reuse guard when the cursor moves.
    cursor_at_render: Option<RendererCursorPos>,
}

impl LokiPageSource {
    pub(crate) fn new(
        cache: Arc<Mutex<PageCache<PageIndex>>>,
        source: Arc<DocPageSource>,
        page_index: usize,
        renderer: Arc<Mutex<Option<vello::Renderer>>>,
        cursor_holder: Arc<Mutex<Option<RendererCursorPos>>>,
    ) -> Self {
        Self {
            cache,
            source,
            page_index,
            renderer,
            device: None,
            wgpu_queue: None,
            font_cache: FontDataCache::new(),
            texture_handle: None,
            texture_tier: None,
            texture_generation: 0,
            texture_size: (0, 0),
            cursor_holder,
            cursor_at_render: None,
        }
    }
}

// ── CustomPaintSource ─────────────────────────────────────────────────────────

impl CustomPaintSource for LokiPageSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.device = Some(device_handle.device.clone());
        self.wgpu_queue = Some(device_handle.queue.clone());

        let mut guard = self.renderer.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            match crate::vello_init::create_vello_renderer(&device_handle.device) {
                Ok(r) => *guard = Some(r),
                Err(e) => tracing::warn!(
                    page = self.page_index,
                    error = %e,
                    "LokiPageSource: vello renderer init failed",
                ),
            }
        }
    }

    fn suspend(&mut self) {
        // Renderer intentionally not dropped on suspend — shared across all page
        // sources; dropped when RendererState is dropped.
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
        let (Some(device), Some(queue)) = (self.device.as_ref(), self.wgpu_queue.as_ref()) else {
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
        let scale_factor = current_tier.scale_factor();
        let w_phys = ((width as f32 * scale_factor).ceil() as u32).max(1);
        let h_phys = ((height as f32 * scale_factor).ceil() as u32).max(1);

        // Step 3: read current document generation.
        let current_generation = self.source.current_generation();

        // Read current cursor position from the shared holder.
        let current_cursor: Option<RendererCursorPos> =
            self.cursor_holder.lock().ok().and_then(|g| *g);

        // Step 4: reuse guard — return existing handle when nothing changed.
        if self.texture_handle.is_some()
            && self.texture_tier == Some(current_tier)
            && self.texture_generation == current_generation
            && self.texture_size == (w_phys, h_phys)
            && self.cursor_at_render == current_cursor
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
            size: Extent3d {
                width: w_phys,
                height: h_phys,
                depth_or_array_layers: 1,
            },
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
        let render_scale = scale as f32 * scale_factor * (96.0 / 72.0);

        // Compute cursor paint data in a scoped block so the layout guard is
        // dropped before the second layout_for_generation call below.
        let cursor_paint = {
            current_cursor.and_then(|cp| {
                if cp.page_index != self.page_index {
                    return None;
                }
                let guard = self.source.layout_for_generation(current_generation);
                // Reflow layouts carry no editing data — no cursor is painted.
                let layout = guard.as_ref()?.1.as_paginated()?;
                let page = layout.pages.get(self.page_index)?;
                let editing_data = page.editing_data.as_ref()?;
                let para_data = editing_data
                    .paragraphs
                    .iter()
                    .find(|p| p.block_index == cp.paragraph_index)?;
                let cursor_rect = para_data.layout.cursor_rect(cp.byte_offset);
                Some(loki_vello::CursorPaint {
                    cursor_rect,
                    selection_rects: vec![],
                    selection_handles: vec![],
                    paragraph_index: cp.paragraph_index,
                })
                // guard drops here, before layout_for_generation is called again
            })
        };

        // Reflow caret: passed as (block_index, byte_offset); paint_tile paints
        // it on whichever band tile it falls in (paginated mode uses the
        // page-relative `cursor_paint` above instead).
        let reflow_cursor = current_cursor.map(|cp| (cp.paragraph_index, cp.byte_offset));

        let layout_guard = self.source.layout_for_generation(current_generation);
        let (_, layout) = layout_guard.as_ref()?;
        let mut scene = Scene::new();
        layout.paint_tile(
            &mut scene,
            &mut self.font_cache,
            self.page_index,
            render_scale,
            cursor_paint.as_ref(),
            reflow_cursor,
        );
        drop(layout_guard);

        // Step 8: render scene to texture.
        // AUDIT: Mutex poisoning on render — lock is held for the duration of
        // render_to_texture; poisoning here would mean the renderer is unusable.
        let mut guard = self.renderer.lock().unwrap_or_else(|p| p.into_inner());
        let renderer = guard.as_mut()?;
        let params = RenderParams {
            base_color: vello::peniko::Color::WHITE,
            width: w_phys,
            height: h_phys,
            // COMPAT(android-mali): area AA on Android — see resume().
            #[cfg(target_os = "android")]
            antialiasing_method: AaConfig::Area,
            #[cfg(not(target_os = "android"))]
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
        drop(guard);

        // Step 9: register with Blitz and cache the handle.
        let handle = ctx.register_texture(texture);
        self.texture_handle = Some(handle.clone());
        self.texture_tier = Some(current_tier);
        self.texture_generation = current_generation;
        self.texture_size = (w_phys, h_phys);
        self.cursor_at_render = current_cursor;

        // Step 10: update cache metadata.
        if let Ok(mut g) = self.cache.lock() {
            g.insert(PageIndex(self.page_index as u32), current_tier);
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
