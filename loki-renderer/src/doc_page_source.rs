// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Generation-aware layout cache bridging `loki-doc-model` to `LokiPageSource`.
//!
//! [`DocPageSource`] wraps an [`Arc<Document>`] and a generation counter.
//! The counter starts at 1; external callers invoke
//! [`DocPageSource::advance_generation`] after a document mutation so that
//! every [`LokiPageSource`] picks up the change on its next frame render.
//!
//! # Layout caching
//!
//! [`DocPageSource::layout_for_generation`] returns a [`MutexGuard`] holding
//! `Option<(u64, PaginatedLayout)>`.  The guard keeps the layout allocation
//! alive without cloning.  If the stored generation differs from the requested
//! generation the layout is recomputed under the same lock acquisition, so the
//! check and the write are atomic with respect to concurrent readers.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use loki_doc_model::document::Document;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout};
use loki_render_cache::page_source::RenderError;
use loki_render_cache::texture::GpuTexture;
use loki_render_cache::{PageIndex, PageSource};
use loki_vello::FontDataCache;
use vello::{AaConfig, AaSupport, RenderParams, RendererOptions, Scene};

// ── A4 page size at 96 dpi ────────────────────────────────────────────────────

/// Default page width in pixels at 96 dpi (A4: 210 mm → ~794 px).
const A4_WIDTH_PX: u32 = 794;
/// Default page height in pixels at 96 dpi (A4: 297 mm → ~1123 px).
const A4_HEIGHT_PX: u32 = 1123;

// ── DocPageSource ─────────────────────────────────────────────────────────────

/// Bridges `loki-doc-model` to the [`PageSource`] trait and `LokiPageSource`.
///
/// Holds a shared reference to the document, a generation counter, and a
/// generation-keyed layout cache.  Multiple [`LokiPageSource`] instances share
/// one `DocPageSource` via [`Arc`]; whichever page renders first after a
/// generation advance causes the layout recompute; the rest reuse the result.
pub struct DocPageSource {
    /// Shared document reference.
    doc: Arc<Document>,
    /// Generation-keyed layout cache.  `None` until first render.
    layout_cache: Mutex<Option<(u64, PaginatedLayout)>>,
    /// Shared font cache for rendering (used by the `PageSource::render` path).
    font_cache: Mutex<FontDataCache>,
    /// Lazily-initialised Vello renderer for the `PageSource::render` path.
    renderer: Mutex<Option<vello::Renderer>>,
    /// Monotone generation counter.  Starts at 1 so that `LokiPageSource`
    /// (whose `texture_generation` initialises to 0) always renders on its
    /// first frame.
    generation: Arc<AtomicU64>,
}

impl DocPageSource {
    /// Creates a new [`DocPageSource`] backed by `doc`.
    pub fn new(doc: Arc<Document>) -> Self {
        Self {
            doc,
            layout_cache: Mutex::new(None),
            font_cache: Mutex::new(FontDataCache::new()),
            renderer: Mutex::new(None),
            generation: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Returns the current document generation.
    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Increments the generation counter.
    ///
    /// Call this after applying a document mutation so that [`LokiPageSource`]
    /// instances re-render on their next frame.
    pub fn advance_generation(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Returns a guard holding the layout for `generation`, recomputing if stale.
    ///
    /// The guard keeps the [`PaginatedLayout`] alive without cloning.
    /// Callers extract `&PaginatedLayout` via:
    /// ```ignore
    /// let guard = source.layout_for_generation(gen);
    /// let Some((_, layout)) = guard.as_ref() else { return; };
    /// ```
    pub fn layout_for_generation(
        &self,
        generation: u64,
    ) -> MutexGuard<'_, Option<(u64, PaginatedLayout)>> {
        let mut guard = self.layout_cache.lock().unwrap_or_else(|e| e.into_inner());
        let needs_recompute = guard.as_ref().map(|(g, _)| *g != generation).unwrap_or(true);
        if needs_recompute {
            let mut resources = FontResources::new();
            let options = LayoutOptions::default();
            let layout = match loki_layout::layout_document(
                &mut resources,
                &self.doc,
                LayoutMode::Paginated,
                1.0,
                &options,
            ) {
                DocumentLayout::Paginated(pl) => pl,
                _ => unreachable!("LayoutMode::Paginated must return DocumentLayout::Paginated"),
            };
            *guard = Some((generation, layout));
        }
        guard
    }
}

// ── PageSource impl ───────────────────────────────────────────────────────────

impl PageSource for DocPageSource {
    fn page_size_px(&self, index: PageIndex) -> (u32, u32) {
        let guard = self.layout_for_generation(self.current_generation());
        guard
            .as_ref()
            .and_then(|(_, layout)| layout.pages.get(index.0 as usize))
            .map(|p| (p.page_size.width.ceil() as u32, p.page_size.height.ceil() as u32))
            .unwrap_or((A4_WIDTH_PX, A4_HEIGHT_PX))
    }

    fn render(
        &self,
        index: PageIndex,
        scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<GpuTexture, RenderError> {
        let span = tracing::debug_span!("DocPageSource::render", index = index.0, scale = scale);
        let _enter = span.enter();
        let t_start = Instant::now();

        // Acquire layout and derive physical dimensions in one lock.
        let generation = self.current_generation();
        let layout_guard = self.layout_for_generation(generation);
        let Some((_, layout)) = layout_guard.as_ref() else {
            return Err(RenderError::Wgpu("layout unavailable".to_string()));
        };
        if index.0 as usize >= layout.pages.len() {
            return Err(RenderError::Wgpu(format!("No such page: {}", index.0)));
        }
        let (base_w, base_h) = layout
            .pages
            .get(index.0 as usize)
            .map(|p| (p.page_size.width.ceil() as u32, p.page_size.height.ceil() as u32))
            .unwrap_or((A4_WIDTH_PX, A4_HEIGHT_PX));
        let w = ((base_w as f32 * scale).ceil() as u32).max(1);
        let h = ((base_h as f32 * scale).ceil() as u32).max(1);

        let inner = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("doc-page-stub"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = inner.create_view(&wgpu::TextureViewDescriptor::default());

        let mut scene = Scene::new();
        let mut font_cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        loki_vello::paint_single_page(
            &mut scene,
            layout,
            &mut font_cache,
            (0.0, 0.0),
            scale,
            index.0 as usize,
            None,
        );
        drop(font_cache);
        drop(layout_guard);

        let mut guard = self.renderer.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            match vello::Renderer::new(
                device,
                RendererOptions {
                    use_cpu: false,
                    antialiasing_support: AaSupport::all(),
                    num_init_threads: NonZeroUsize::new(1),
                    pipeline_cache: None,
                },
            ) {
                Ok(r) => *guard = Some(r),
                Err(e) => return Err(RenderError::Wgpu(e.to_string())),
            }
        }
        let Some(renderer) = guard.as_mut() else {
            return Err(RenderError::Wgpu("renderer unavailable".to_string()));
        };

        let params = RenderParams {
            base_color: vello::peniko::Color::WHITE,
            width: w,
            height: h,
            antialiasing_method: AaConfig::Area,
        };
        renderer
            .render_to_texture(device, queue, &scene, &view, &params)
            .map_err(|e| RenderError::Wgpu(e.to_string()))?;

        tracing::debug!(
            index = index.0,
            scale = scale,
            width_px = w,
            height_px = h,
            elapsed_ms = t_start.elapsed().as_millis(),
            "page rendered",
        );

        Ok(GpuTexture { inner, width: w, height: h })
    }
}
