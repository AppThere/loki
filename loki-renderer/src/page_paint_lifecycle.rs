// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `LokiPageSource` construction and lifecycle — everything except the render
//! pass, split out of `page_paint_source.rs` for the 300-line ceiling.
//!
//! The cohesive cluster here is **texture ownership**: creating a source with
//! no texture, acquiring the device, and the two paths that give a texture back
//! (`on_suspend`, `on_release`). Keeping them together puts every release site
//! in one file, which matters because the Phase 2 residency counter is only
//! correct if every one of them records its free — see
//! [`LokiPageSource::record_texture_released`].
//!
//! These are inherent methods; the `CustomPaintSource` impl in the parent
//! delegates to them, because a trait impl cannot be split across modules.

use std::sync::{Arc, Mutex};

use anyrender_vello::{CustomPaintCtx, DeviceHandle};
use appthere_canvas::residency::{TextureResidency, texture_bytes};

use super::LokiPageSource;
use crate::doc_page_source::DocPageSource;
use crate::document_view::RendererSelection;

impl LokiPageSource {
    pub(crate) fn new(
        source: Arc<DocPageSource>,
        page_index: usize,
        renderer: Arc<Mutex<Option<vello::Renderer>>>,
        cursor_holder: Arc<Mutex<Option<RendererSelection>>>,
    ) -> Self {
        Self {
            source,
            page_index,
            renderer,
            device: None,
            wgpu_queue: None,
            texture_handle: None,
            texture_generation: 0,
            texture_size: (0, 0),
            cursor_holder,
            cursor_at_render: None,
        }
    }

    /// Records the release of this source's texture with the Phase 2 residency
    /// counter and forgets its size (Spec 08 T2.5).
    ///
    /// Every path that drops the texture — [`Self::on_suspend`],
    /// [`Self::on_release`], and the re-render in `render` — calls this exactly
    /// once, because an allocation without its matching free does not merely
    /// mis-report: the counter drifts upward for the rest of the session, so a
    /// budget check made later fails for a reason unrelated to the pages
    /// actually mounted.
    ///
    /// Idempotent by construction: it takes the recorded size, so a second call
    /// finds `(0, 0)` and does nothing. That matters because `suspend` and
    /// `release` are both reachable for the same source.
    pub(super) fn record_texture_released(&mut self) {
        let (w, h) = std::mem::take(&mut self.texture_size);
        if w > 0 && h > 0 {
            TextureResidency::record_free(texture_bytes(w, h));
        }
    }

    /// `CustomPaintSource::resume` — take the device, record what GPU we landed
    /// on, and make sure the shared Vello renderer exists.
    pub(super) fn on_resume(&mut self, device_handle: &DeviceHandle) {
        self.device = Some(device_handle.device.clone());
        self.wgpu_queue = Some(device_handle.queue.clone());
        // T2.0: the adapter Blitz actually chose, which is the one a texture
        // budget must be sized against. `get_info()` is a cheap read of cached
        // adapter metadata, and every source resumes on the same adapter, so
        // repeating it per tile costs nothing and needs no first-time guard.
        crate::gpu_probe::record(crate::gpu_probe::kind_of(
            device_handle.adapter.get_info().device_type,
        ));

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

    /// `CustomPaintSource::suspend` — the app is going away.
    pub(super) fn on_suspend(&mut self) {
        // Renderer intentionally not dropped on suspend — shared across all page
        // sources; dropped when RendererState is dropped.
        //
        // The texture handle is cleared here without unregistering it from the
        // renderer because suspend() has no CustomPaintCtx. That is safe for the
        // app-level suspend path (the window renderer is recreated on resume,
        // dropping every registered texture). Per-source teardown — a tile
        // scrolling out of the virtualization window — instead goes through
        // `on_release` below, which DOES unregister the texture; otherwise each
        // unmounted page would leak its full-resolution texture (~10+ MB) in the
        // renderer's registry, growing RAM without bound as the user scrolls.
        self.device = None;
        self.wgpu_queue = None;
        self.texture_handle = None;
        self.texture_generation = 0;
        // The window renderer is recreated on resume, dropping every registered
        // texture — so the GPU memory does go, and the counter must see it.
        self.record_texture_released();
    }

    /// `CustomPaintSource::release` — this tile alone is going away, while the
    /// renderer stays live.
    pub(super) fn on_release(&mut self, mut ctx: CustomPaintCtx<'_>) {
        // The tile is being unregistered while the renderer is still live, so
        // free the GPU texture this source registered. Without this the texture
        // outlives the source in the renderer's registry (see on_suspend()).
        if let Some(handle) = self.texture_handle.take() {
            ctx.unregister_texture(handle);
        }
        self.texture_generation = 0;
        self.record_texture_released();
    }
}
