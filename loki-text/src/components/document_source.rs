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
use kurbo::Rect;
use loki_doc_model::document::Document;
use loki_layout::{layout_document, DocumentLayout, FontResources, LayoutMode};
use loki_vello::{paint_layout, FontDataCache};
use peniko::Color;
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
    /// Number of pages in the current paginated layout; 0 when no document is loaded.
    pub page_count: usize,
    /// Visible viewport in document-space coordinates — future partial-render
    /// seam.  Set to `None` until scroll infrastructure is implemented.
    pub visible_rect: Option<Rect>,
}

// ── Cached layout ─────────────────────────────────────────────────────────────

struct CachedLayout {
    generation: u64,
    layout: DocumentLayout,
    font_cache: FontDataCache,
}

// ── LokiDocumentSource ────────────────────────────────────────────────────────

/// `CustomPaintSource` that renders one page of a [`Document`] to a wgpu texture each frame.
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
    /// Index of the page this source renders (0-based).
    page_index: usize,
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
    /// Create a new source for `page_index`, sharing state with the Dioxus component.
    pub(crate) fn new(document: Arc<Mutex<DocumentState>>, page_index: usize) -> Self {
        Self {
            document,
            page_index,
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

        // Phase 1: Read document state under lock, then release before layout work.
        // Cloning the document avoids a borrow conflict when we later write
        // page_count back to state (can't hold an immutable borrow of
        // state.document while mutably borrowing state.page_count through the
        // same MutexGuard).
        let (doc_opt, current_gen) = {
            let state = match self.document.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("LokiDocumentSource: document lock poisoned: {e}");
                    return None;
                }
            };
            (state.document.clone(), state.generation)
        };

        // No document loaded — WgpuSurface shows a placeholder div instead.
        let doc = doc_opt?;

        // Phase 2: Rebuild paginated layout when generation advances.
        if self.needs_relayout(current_gen) {
            let font_resources = self.font_resources.get_or_insert_with(FontResources::new);
            let layout =
                layout_document(font_resources, &doc, LayoutMode::Paginated, scale as f32);
            let page_count = match &layout {
                DocumentLayout::Paginated(pl) => pl.pages.len(),
                _ => 0,
            };
            self.layout_cache = Some(CachedLayout {
                generation: current_gen,
                layout,
                font_cache: FontDataCache::new(),
            });

            // Phase 3: Publish page_count to shared state (used by toolbar and
            // WgpuSurface for display purposes).
            if let Ok(mut state) = self.document.lock() {
                state.page_count = page_count;
            }
        }

        // Phase 4: Paint this page's scene.
        let mut scene = Scene::new();
        // TODO(partial-render): pass visible_rect as clip region to paint_layout
        // when the partial render pipeline is implemented.
        let cached = self.layout_cache.as_mut()?;
        paint_layout(
            &mut scene,
            &cached.layout,
            &mut cached.font_cache,
            (0.0, 0.0),
            scale as f32,
            Some(self.page_index),
        );

        // Phase 5: GPU work — borrow GPU resources mutably only after all
        // immutable self-borrows above are complete (borrow checker requires
        // non-overlapping borrows on self).
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
        LokiDocumentSource::new(
            Arc::new(Mutex::new(DocumentState {
                document: None,
                generation: 0,
                page_count: 0,
                visible_rect: None,
            })),
            0,
        )
    }

    /// Constructs a `CachedLayout` by running the real layout pipeline on an
    /// empty document — avoids constructing `DocumentLayout` directly (non_exhaustive).
    fn make_cached_layout(generation: u64) -> CachedLayout {
        let doc = Document::new();
        let mut resources = FontResources::new();
        let layout = layout_document(&mut resources, &doc, LayoutMode::Paginated, 1.0);
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
            page_count: 0,
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
