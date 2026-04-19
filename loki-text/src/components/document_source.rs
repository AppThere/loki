// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! GPU paint source for Loki document rendering.
//!
//! [`LokiDocumentSource`] implements [`anyrender_vello::CustomPaintSource`], the
//! officially supported extension point for injecting custom Vello scenes into
//! Blitz's render loop.  It is registered once via `dioxus::native::use_wgpu`
//! and called each frame by `anyrender_vello::VelloWindowRenderer`.
//!
//! Document state is shared with the Dioxus component via
//! `Arc<Mutex<DocumentState>>`.  A generation counter avoids redundant
//! `layout_document` calls on frames where nothing has changed.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use anyrender_vello::wgpu::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use kurbo::{Affine, Rect, Stroke};
use loki_doc_model::document::Document;
use loki_layout::{layout_document, DocumentLayout, FontResources, LayoutMode};
use loki_theme::tokens;
use loki_vello::{paint_layout, FontDataCache};
use peniko::{Brush, Color, Fill};
use vello::{AaConfig, AaSupport, RenderParams, RendererOptions, Scene};

// ── Shared state ──────────────────────────────────────────────────────────────

/// Document state shared between the Dioxus component and [`LokiDocumentSource`].
///
/// The Dioxus component updates this on every render cycle; `LokiDocumentSource`
/// reads it inside `render()`.  `Mutex` serialises access between the virtual-DOM
/// update thread and the GPU render thread.
pub struct DocumentState {
    /// Currently loaded document, or `None` when no file is open.
    pub document: Option<Document>,
    /// Bumped each time `document` changes; drives layout-cache invalidation.
    pub generation: u64,
    /// Visible viewport in document-space coordinates — future partial-render
    /// seam.  Set to `None` until scroll infrastructure is implemented.
    pub visible_rect: Option<Rect>,
}

// ── Cached layout ──────────────────────────────────────────────────────────────

struct CachedLayout {
    generation: u64,
    layout: DocumentLayout,
    font_cache: FontDataCache,
}

// ── LokiDocumentSource ────────────────────────────────────────────────────────

/// `CustomPaintSource` that renders a [`Document`] to a wgpu texture each frame.
///
/// Lifecycle:
/// - `resume()` — GPU device is available; create `vello::Renderer` and
///   `FontResources`.
/// - `render()` — called each frame by Blitz; runs layout + paint → texture →
///   `ctx.register_texture`.
/// - `suspend()` — GPU device is lost; drop GPU resources, retain
///   `font_resources`.
pub(crate) struct LokiDocumentSource {
    /// Shared document state — updated by the Dioxus component when props change.
    document: Arc<Mutex<DocumentState>>,
    /// wgpu device, cloned from [`DeviceHandle`] in `resume()`.
    device: Option<anyrender_vello::wgpu::Device>,
    /// wgpu queue, cloned from [`DeviceHandle`] in `resume()`.
    queue: Option<anyrender_vello::wgpu::Queue>,
    /// Own Vello renderer — created in `resume()` from the device.
    renderer: Option<vello::Renderer>,
    /// Cached layout — invalidated when the generation counter advances.
    layout_cache: Option<CachedLayout>,
    /// Font resources — initialized in `resume()`, persisted across frames to
    /// avoid re-scanning system fonts on every render call.
    font_resources: Option<FontResources>,
}

impl LokiDocumentSource {
    /// Create a new source sharing state with the Dioxus component.
    pub(crate) fn new(document: Arc<Mutex<DocumentState>>) -> Self {
        Self {
            document,
            device: None,
            queue: None,
            renderer: None,
            layout_cache: None,
            font_resources: None,
        }
    }

    /// Returns `true` if `layout_cache` must be rebuilt for `generation`.
    fn needs_relayout(&self, generation: u64) -> bool {
        self.layout_cache
            .as_ref()
            .map_or(true, |c| c.generation != generation)
    }

    /// Build a blank A4 placeholder scene (white fill + 1 px border).
    fn build_placeholder_scene(scene: &mut Scene) {
        let page = Rect::new(
            0.0,
            0.0,
            tokens::PAGE_WIDTH_PX as f64,
            tokens::PAGE_HEIGHT_PX as f64,
        );
        let white = Brush::Solid(Color::new([1.0_f32, 1.0, 1.0, 1.0]));
        scene.fill(Fill::NonZero, Affine::IDENTITY, &white, None, &page);
        // 1 px border (#E0E0E0).
        let border = Brush::Solid(Color::new([0.878_f32, 0.878, 0.878, 1.0]));
        scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, &border, None, &page);
    }
}

// ── CustomPaintSource impl ────────────────────────────────────────────────────

impl CustomPaintSource for LokiDocumentSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.device = Some(device_handle.device.clone());
        self.queue = Some(device_handle.queue.clone());

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
            Err(e) => {
                tracing::warn!("LokiDocumentSource: vello renderer init failed: {e}");
                self.renderer = None;
            }
        }

        self.font_resources = Some(FontResources::new());
    }

    fn suspend(&mut self) {
        self.device = None;
        self.queue = None;
        self.renderer = None;
        self.layout_cache = None;
        // font_resources is retained — it has no GPU dependency.
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        // Guard: GPU resources must be present.
        if self.device.is_none() || self.queue.is_none() || self.renderer.is_none() {
            return None;
        }

        let state = match self.document.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!("LokiDocumentSource: document lock poisoned: {e}");
                return None;
            }
        };

        let mut scene = Scene::new();

        if let Some(doc) = state.document.as_ref() {
            let current_gen = state.generation;

            // needs_relayout borrows self immutably; perform before any &mut self
            // borrow so the borrow checker sees no overlap with renderer.as_mut().
            if self.needs_relayout(current_gen) {
                let font_resources =
                    self.font_resources.get_or_insert_with(FontResources::new);
                let layout =
                    layout_document(font_resources, doc, LayoutMode::Pageless, scale as f32);
                self.layout_cache = Some(CachedLayout {
                    generation: current_gen,
                    layout,
                    font_cache: FontDataCache::new(),
                });
            }

            // TODO(partial-render): pass visible_rect as clip region to paint_layout
            // when the partial render pipeline is implemented.
            if let Some(cached) = self.layout_cache.as_mut() {
                paint_layout(
                    &mut scene,
                    &cached.layout,
                    &mut cached.font_cache,
                    (0.0, 0.0),
                    scale as f32,
                );
            }
        } else {
            Self::build_placeholder_scene(&mut scene);
        }

        // Release the mutex before GPU work to minimise contention.
        drop(state);

        // Borrow GPU resources mutably only after all immutable self-borrows above
        // are complete (borrow checker requires non-overlapping borrows on self).
        let device = self.device.as_ref()?;
        let queue = self.queue.as_ref()?;
        let renderer = self.renderer.as_mut()?;

        // COMPAT(blitz): Rgba8Unorm with STORAGE_BINDING|TEXTURE_BINDING is the
        // format expected by vello render_to_texture and register_texture in
        // anyrender_vello 0.6.2. If compositing produces garbage pixels, inspect
        // VelloRenderer::register_texture for format validation.
        let texture = device.create_texture(&anyrender_vello::wgpu::TextureDescriptor {
            label: Some("loki_document_source"),
            size: Extent3d {
                width,
                height,
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
        let params = RenderParams {
            base_color: Color::WHITE,
            width,
            height,
            antialiasing_method: AaConfig::Msaa16,
        };

        if let Err(e) = renderer.render_to_texture(device, queue, &scene, &view, &params) {
            tracing::error!("LokiDocumentSource: render_to_texture failed: {e}");
            return None;
        }

        Some(ctx.register_texture(texture))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::document::Document;
    use loki_layout::LayoutMode;

    fn make_source() -> LokiDocumentSource {
        LokiDocumentSource::new(Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            visible_rect: None,
        })))
    }

    /// Constructs a `CachedLayout` by running the real layout pipeline on an
    /// empty document — avoids constructing `DocumentLayout` directly (non_exhaustive).
    fn make_cached_layout(generation: u64) -> CachedLayout {
        let doc = Document::new();
        let mut resources = FontResources::new();
        let layout = layout_document(&mut resources, &doc, LayoutMode::Pageless, 1.0);
        CachedLayout {
            generation,
            layout,
            font_cache: FontDataCache::new(),
        }
    }

    #[test]
    fn layout_cache_initially_empty() {
        assert!(make_source().layout_cache.is_none());
    }

    #[test]
    fn needs_relayout_when_cache_empty() {
        let source = make_source();
        assert!(source.needs_relayout(0));
        assert!(source.needs_relayout(42));
    }

    #[test]
    fn no_relayout_when_generation_matches() {
        let mut source = make_source();
        source.layout_cache = Some(make_cached_layout(7));
        assert!(!source.needs_relayout(7), "same generation → no relayout");
    }

    #[test]
    fn relayout_when_generation_advances() {
        let mut source = make_source();
        source.layout_cache = Some(make_cached_layout(7));
        assert!(source.needs_relayout(8), "advanced generation → relayout");
    }

    #[test]
    fn generation_counter_increments_on_document_change() {
        let state = Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            visible_rect: None,
        }));
        // Simulate the component bumping the generation counter.
        {
            let mut s = state.lock().unwrap();
            s.document = Some(Document::new());
            s.generation = s.generation.wrapping_add(1);
        }
        assert_eq!(state.lock().unwrap().generation, 1);
    }
}
