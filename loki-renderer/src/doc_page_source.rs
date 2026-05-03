// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! [`PageSource`] bridge over `loki-doc-model`.
//!
//! [`DocPageSource`] implements the [`PageSource`] trait so that
//! `loki-render-cache`'s [`RenderQueue`](loki_render_cache::RenderQueue) can
//! drive rendering without depending on `loki-doc-model` directly.
//!
//! # Render stub (Session 4)
//!
//! The [`PageSource::render`] implementation builds a solid-white Vello scene
//! and renders it to a GPU texture.  Full document-content rendering is
//! Session 5 scope.  The stub is deliberately minimal so that the wgpu path
//! exercised by Session 3's integration tests remains the primary coverage
//! vehicle this session.
//!
//! # Vello renderer lifecycle
//!
//! `vello::Renderer` compiles GPU shaders on first construction.  To avoid
//! rebuilding it on every [`PageSource::render`] call, [`DocPageSource`]
//! holds a `Mutex<Option<vello::Renderer>>` and lazily initialises it using
//! the `wgpu::Device` supplied by the render-queue worker.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use loki_doc_model::document::Document;
use loki_render_cache::page_source::RenderError;
use loki_render_cache::texture::GpuTexture;
use loki_render_cache::{PageIndex, PageSource};
use vello::{AaConfig, AaSupport, RenderParams, RendererOptions, Scene};
use loki_layout::{PaginatedLayout, FontResources, LayoutOptions, LayoutMode, DocumentLayout};
use loki_vello::FontDataCache;

// ── A4 page size at 96 dpi ────────────────────────────────────────────────────

/// Default page width in pixels at 96 dpi (A4: 210 mm → ~794 px).
const A4_WIDTH_PX: u32 = 794;
/// Default page height in pixels at 96 dpi (A4: 297 mm → ~1123 px).
const A4_HEIGHT_PX: u32 = 1123;

// ── DocPageSource ─────────────────────────────────────────────────────────────

/// Bridges `loki-doc-model` to the [`PageSource`] trait.
///
/// Holds a shared reference to the document so that multiple renderer threads
/// can call [`PageSource::render`] concurrently.  `Document` is `Send + Sync`
/// (no interior mutability), satisfying the `PageSource: Send + Sync` bound.
///
/// The lazy [`vello::Renderer`] is protected by a [`Mutex`]; it is created
/// from the `wgpu::Device` on the first `render` call and reused thereafter.
pub struct DocPageSource {
    /// Shared document reference.
    doc: Arc<Document>,
    /// Layout generated once per document modification.
    layout_cache: OnceLock<PaginatedLayout>,
    /// Shared font cache for layout and rendering.
    font_cache: Mutex<FontDataCache>,
    /// Lazily-initialised Vello renderer, cached to avoid shader recompilation.
    renderer: Mutex<Option<vello::Renderer>>,
}

impl DocPageSource {
    /// Creates a new [`DocPageSource`] backed by `doc`.
    pub fn new(doc: Arc<Document>) -> Self {
        Self {
            doc,
            layout_cache: OnceLock::new(),
            font_cache: Mutex::new(FontDataCache::new()),
            renderer: Mutex::new(None),
        }
    }

    /// Lazily computes and returns the paginated layout.
    pub fn layout(&self) -> &PaginatedLayout {
        self.layout_cache.get_or_init(|| {
            let mut resources = FontResources::new();
            let options = LayoutOptions::default();
            match loki_layout::layout_document(
                &mut resources,
                &self.doc,
                LayoutMode::Paginated,
                1.0,
                &options,
            ) {
                DocumentLayout::Paginated(pl) => pl,
                _ => unreachable!("LayoutMode::Paginated must return DocumentLayout::Paginated"),
            }
        })
    }
}

// ── PageSource impl ───────────────────────────────────────────────────────────

impl PageSource for DocPageSource {
    /// Returns the logical page size in pixels at 1× scale.
    ///
    /// For Session 4 this returns the A4 default (794 × 1123 px).
    /// Session 5 will derive dimensions from the first section's `PageLayout`.
    fn page_size_px(&self, index: PageIndex) -> (u32, u32) {
        self.layout()
            .pages
            .get(index.0 as usize)
            .map(|p| (p.page_size.width.ceil() as u32, p.page_size.height.ceil() as u32))
            .unwrap_or((A4_WIDTH_PX, A4_HEIGHT_PX))
    }

    /// Renders `index` at `scale × page_size_px` to a GPU texture.
    ///
    /// Session 4 stub: fills the page with solid white.  A
    /// [`tracing::debug_span`] wraps the full render call, logging `index`,
    /// `scale`, and elapsed milliseconds.
    fn render(
        &self,
        index: PageIndex,
        scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<GpuTexture, RenderError> {
        let span = tracing::debug_span!(
            "DocPageSource::render",
            index = index.0,
            scale = scale,
        );
        let _enter = span.enter();
        let t_start = Instant::now();

        let (base_w, base_h) = self.page_size_px(index);
        let w = ((base_w as f32 * scale).ceil() as u32).max(1);
        let h = ((base_h as f32 * scale).ceil() as u32).max(1);

        // Allocate a texture with the usages Vello's render_to_texture expects.
        // `allocate_texture` in loki-render-cache uses RENDER_ATTACHMENT (for
        // the blit pipeline); Vello needs STORAGE_BINDING instead, so we
        // create the texture manually here.
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

        let layout = self.layout();
        if index.0 as usize >= layout.pages.len() {
            return Err(RenderError::Wgpu(format!("No such page: {}", index.0)));
        }

        // Build the scene using loki-vello.
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

        // Lazily create or reuse the Vello renderer.
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

        let elapsed_ms = t_start.elapsed().as_millis();
        tracing::debug!(
            index = index.0,
            scale = scale,
            width_px = w,
            height_px = h,
            elapsed_ms = elapsed_ms,
            "page rendered",
        );

        Ok(GpuTexture { inner, width: w, height: h })
    }
}
