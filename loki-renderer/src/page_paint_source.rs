// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-page GPU paint source for `DocumentView`.
//!
//! [`LokiPageSource`] implements [`CustomPaintSource`] so that Blitz's frame
//! loop drives rendering.  On each frame it:
//!
//! 1. Reuses the registered [`TextureHandle`] when the document generation,
//!    physical size, and cursor are all unchanged (zero re-render cost).
//! 2. Otherwise unregisters the old texture, re-renders via Vello, and
//!    registers the fresh texture with Blitz.
//!
//! Every mounted tile renders at full resolution; virtualization only mounts
//! pages near the viewport, so texture memory is bounded by mounting.

use std::sync::{Arc, Mutex};

use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use appthere_canvas::residency::{TextureResidency, texture_bytes};
use vello::{AaConfig, RenderParams, Scene};

use crate::doc_page_source::DocPageSource;
use crate::document_view::RendererSelection;

#[path = "page_paint_render.rs"]
mod render;

#[path = "page_paint_lifecycle.rs"]
mod lifecycle;

/// Set once, when the first page tile is rendered, to attribute Vello's one-time
/// pipeline warm-up to the open-path timing log (see the render path below).
static FIRST_TILE_RENDERED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

// ── LokiPageSource ────────────────────────────────────────────────────────────

pub(crate) struct LokiPageSource {
    /// Document layout + page-size source.
    source: Arc<DocPageSource>,
    /// 0-based page index this source renders.
    page_index: usize,
    // COMPAT(loki): the first page source to resume creates the shared Vello
    // renderer; subsequent sources find it populated and skip creation.
    renderer: Arc<Mutex<Option<vello::Renderer>>>,
    /// wgpu device from the last `resume()`.
    device: Option<anyrender_vello::wgpu::Device>,
    /// wgpu queue from the last `resume()`.
    wgpu_queue: Option<anyrender_vello::wgpu::Queue>,
    /// Currently registered Blitz texture handle.
    texture_handle: Option<TextureHandle>,
    /// Document generation at which `texture_handle` was rendered.
    texture_generation: u64,
    /// Physical pixel dimensions `(w, h)` of `texture_handle`.
    texture_size: (u32, u32),
    /// Shared cursor position written by PageTile on every Dioxus render.
    cursor_holder: Arc<Mutex<Option<RendererSelection>>>,
    /// Cursor at last render — invalidates the reuse guard on cursor moves.
    cursor_at_render: Option<RendererSelection>,
}

// ── CustomPaintSource ─────────────────────────────────────────────────────────

// A trait impl cannot be split across modules, so the lifecycle half delegates
// to inherent methods in `lifecycle`; `render` — the substantive one — stays
// here. See `page_paint_lifecycle.rs` for why those three group together.
impl CustomPaintSource for LokiPageSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.on_resume(device_handle);
    }

    fn suspend(&mut self) {
        self.on_suspend();
    }

    fn release(&mut self, ctx: CustomPaintCtx<'_>) {
        self.on_release(ctx);
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        // Cloned rather than borrowed: `wgpu::Device`/`Queue` are handles over
        // shared state, so this is a refcount bump, and holding them by value
        // frees `self` for the residency bookkeeping below (which needs `&mut`).
        let (Some(device), Some(queue)) = (self.device.clone(), self.wgpu_queue.clone()) else {
            return None;
        };

        // Step 1: target physical texture dimensions. Every mounted tile renders
        // at full resolution — the canvas's own physical pixel size — so the
        // texture is 1:1 with what Blitz composites; virtualization bounds memory
        // by limiting which pages mount.
        let w_phys = width.max(1);
        let h_phys = height.max(1);

        // Step 2: read current document generation.
        let current_generation = self.source.current_generation();

        // Read the current caret + selection from the shared holder.
        let current_sel: Option<RendererSelection> =
            self.cursor_holder.lock().ok().and_then(|g| *g);

        // Step 3: reuse guard — return existing handle when nothing changed.
        if self.texture_handle.is_some()
            && self.texture_generation == current_generation
            && self.texture_size == (w_phys, h_phys)
            && self.cursor_at_render == current_sel
        {
            return self.texture_handle.clone();
        }

        // Step 4: unregister stale texture before reallocating. Recorded before
        // the new allocation, matching the real order: the old texture is
        // released first, so a zoom change does not transiently double-count.
        if let Some(old) = self.texture_handle.take() {
            ctx.unregister_texture(old);
        }
        self.record_texture_released();

        // Step 6: allocate new GPU texture.
        let (texture, view) = render::allocate_page_texture(&device, w_phys, h_phys);
        // Every early return between here and registration abandons that
        // texture — it is dropped at end of scope, so the GPU memory goes but
        // the counter would not see it, and the drift is permanent. Recorded at
        // each such site rather than trusted to a reviewer, because the failure
        // is invisible: residency simply reads high forever after.
        let abandon = || TextureResidency::record_free(texture_bytes(w_phys, h_phys));

        // Step 6: build Vello scene for this page.
        let render_scale = scale as f32 * (96.0 / 72.0) * self.source.zoom();

        // Compute cursor paint data (scoped so its layout guard is dropped
        // before the second layout_for_generation call below).
        let cursor_paint = render::page_cursor_paint(
            &self.source,
            self.page_index,
            current_generation,
            current_sel,
        );

        // Reflow caret + selection, as (block_index, byte_offset) pairs;
        // paint_tile paints them on whichever band tile they fall in (paginated
        // mode uses the page-relative `cursor_paint` above instead).
        let reflow_cursor =
            current_sel.map(|sel| (sel.focus.paragraph_index, sel.focus.byte_offset));
        let reflow_selection = current_sel.filter(|sel| !sel.is_collapsed()).map(|sel| {
            (
                (sel.anchor.paragraph_index, sel.anchor.byte_offset),
                (sel.focus.paragraph_index, sel.focus.byte_offset),
            )
        });

        let paint_start = std::time::Instant::now();
        let layout_guard = self.source.layout_for_generation(current_generation);
        let Some((_, layout)) = layout_guard.as_ref() else {
            abandon();
            return None;
        };
        let mut scene = Scene::new();
        // One FontDataCache per document, shared across all page tiles via
        // DocPageSource (memory F5 / BM-9) — tiles no longer each clone the
        // face bytes. Tiles render serially, so the lock is uncontended.
        let mut font_cache = self
            .source
            .font_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        layout.paint_tile(
            &mut scene,
            &mut font_cache,
            self.page_index,
            render_scale,
            cursor_paint.as_ref(),
            reflow_cursor,
            reflow_selection,
        );
        drop(font_cache);
        drop(layout_guard);
        let scene_ms = paint_start.elapsed().as_secs_f64() * 1000.0;
        let render_start = std::time::Instant::now();

        // Step 8: render scene to texture.
        // AUDIT: Mutex poisoning on render — lock is held for the duration of
        // render_to_texture; poisoning here would mean the renderer is unusable.
        let mut guard = self.renderer.lock().unwrap_or_else(|p| p.into_inner());
        let Some(renderer) = guard.as_mut() else {
            abandon();
            return None;
        };
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
        if let Err(e) = renderer.render_to_texture(&device, &queue, &scene, &view, &params) {
            tracing::error!(
                page = self.page_index,
                error = %e,
                "LokiPageSource: render_to_texture failed",
            );
            abandon();
            return None;
        }
        drop(guard);

        // Open-path timing: the first tile rendered carries Vello's one-time
        // pipeline/shader compilation, so it dwarfs later tiles. Logged once so
        // the GPU first-paint share of open latency is visible on-device.
        if !FIRST_TILE_RENDERED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::info!(
                target: "loki_text::open",
                page = self.page_index,
                scene_build_ms = scene_ms,
                gpu_render_ms = render_start.elapsed().as_secs_f64() * 1000.0,
                "first page tile rendered (includes Vello pipeline warm-up)",
            );
        }

        // Step 7: register with Blitz and record the reuse-guard state.
        let handle = ctx.register_texture(texture);
        self.texture_handle = Some(handle.clone());
        self.texture_generation = current_generation;
        self.texture_size = (w_phys, h_phys);
        self.cursor_at_render = current_sel;

        tracing::debug!(
            page = self.page_index,
            w = w_phys,
            h = h_phys,
            "LokiPageSource: rendered",
        );

        Some(handle)
    }
}
