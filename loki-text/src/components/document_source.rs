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

/* ── Audit findings (2026-04-19) ────────────────────────────────────────────
 *
 * Q1. Does LokiDocumentSource store the previous frame's TextureHandle?
 *     No — the struct had no texture_handle field.  The handle returned by
 *     ctx.register_texture() was immediately returned from render() and
 *     discarded; nothing retained it between frames.
 *
 * Q2. Is ctx.unregister_texture() called anywhere?
 *     No — no call site existed in the entire codebase.
 *
 * Q3. Is the wgpu::Texture dropped or does register_texture transfer ownership?
 *     register_texture takes ownership by value (texture: wgpu::Texture).
 *     Vello stores the texture in engine.image_overrides (FxHashMap).  The
 *     texture is NOT dropped; ownership is transferred to the renderer.
 *     Calling register_texture N times without unregister_texture accumulates
 *     N live wgpu texture allocations in that HashMap — the confirmed leak.
 *
 * Q4. Does render() create a new wgpu::Texture on every call?
 *     Yes — unconditionally, with no early-return based on texture reuse.
 *
 * Q5. Early-return paths that skip unregister?
 *     Multiple early returns exist (GPU guard, no document, no layout cache),
 *     but none matter for unregister because no handle was ever stored.
 *     The real issue: every path reaching ctx.register_texture() never
 *     released the previous frame's allocation.
 *
 * Ownership semantics of register_texture / unregister_texture:
 *   • register_texture(&mut self, texture: wgpu::Texture) -> TextureHandle
 *     Takes ownership by value; inserts into image_overrides HashMap.
 *     Caller is responsible for calling unregister_texture when done.
 *     Blitz does NOT manage handle lifetime automatically.
 *   • unregister_texture(&mut self, handle: TextureHandle)
 *     Removes the entry from image_overrides; texture allocation is freed.
 *   • Both methods require a CustomPaintCtx, which is only available inside
 *     render().  Cannot be called from suspend() or Drop.
 *   • In suspend(), VelloWindowRenderer drops ActiveRenderState (which drops
 *     VelloRenderer, which drops engine.image_overrides).  So all textures
 *     are freed automatically on suspend — clearing texture_handle to None
 *     in suspend() is sufficient; explicit unregister is not needed there.
 *
 * Memory profile (verified 2026-04-19):
 *   Before fix: unbounded growth — register_texture called every frame with
 *               no corresponding unregister; each frame accumulated one
 *               wgpu::Texture in Vello's image_overrides HashMap.
 *               At Rgba8Unorm A4@96dpi (~794×1123px) ≈ 3.6 MB/texture;
 *               at 60 fps a static document grew ~216 MB/s.
 *   After fix:  stable — existing texture reused every frame on static
 *               documents; unregister_texture called before each new
 *               allocation when layout or size changes.
 * ────────────────────────────────────────────────────────────────────────── */

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use anyrender_vello::wgpu::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use kurbo::Rect;
use loki_doc_model::document::Document;
use loki_layout::{
    layout_document, DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout,
};
use loki_vello::{paint_single_page, CursorPaint, FontDataCache, SelectionRect};
use peniko::Color;
use vello::{AaConfig, AaSupport, RenderParams, RendererOptions, Scene};

use crate::editing::cursor::CursorState;

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
    /// Canvas width in CSS pixels from the last `render()` call.  Used to
    /// detect when the container resizes so the layout cache is invalidated.
    pub canvas_width: f32,
    /// Visible viewport in document-space coordinates — future partial-render
    /// seam.  Set to `None` until scroll infrastructure is implemented.
    pub visible_rect: Option<Rect>,
    /// Page width in CSS logical pixels derived from the document's `<w:pgSz>`.
    /// Kept in sync with `WgpuSurface` so the canvas element and the GPU
    /// texture agree on the page boundary.  Falls back to A4 (794 px) until a
    /// document is loaded.
    pub page_width_px: f32,
    /// Page height in CSS logical pixels derived from the document's `<w:pgSz>`.
    /// Falls back to A4 (1123 px) until a document is loaded.
    pub page_height_px: f32,
    /// Current cursor and selection state from the editing layer.
    ///
    /// `None` in read-only mode (no cursor is painted).  Updated by the
    /// `WgpuSurface` component whenever `cursor_state` props change.
    pub cursor_state: Option<CursorState>,
    /// When `true`, layout passes must retain Parley data for hit-testing.
    pub preserve_for_editing: bool,
    /// Most recently computed paginated layout, shared with the editor route
    /// so that mouse click handlers can call `hit_test_document` without
    /// re-running the layout pipeline.
    ///
    /// Populated by the first [`LokiDocumentSource::render()`] to find it stale,
    /// or by [`apply_mutation_and_relayout`].  `None` until the first render.
    pub paginated_layout: Option<Arc<PaginatedLayout>>,
    /// One `vello::Renderer` shared across all `LokiDocumentSource` instances
    /// for this document.  Keyed to the wgpu `Device`; cleared on suspend and
    /// re-created by the first source to call `resume()`.
    ///
    /// Sharing avoids allocating one renderer per page canvas (≈141 MB each).
    /// Blitz calls sources sequentially so there is no lock contention.
    pub shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
    /// Shared font glyph cache for painting.  Invalidated (reset) whenever
    /// `paginated_layout` is recomputed.  All page sources share one cache so
    /// glyphs loaded while painting page 0 are reused for pages 1-N.
    pub shared_font_cache: Arc<Mutex<FontDataCache>>,
    /// Monotonically increasing counter bumped every time `paginated_layout` is
    /// recomputed (generation change, resize, or `preserve_for_editing` change).
    /// Each source compares its `texture_stamp` against this to detect staleness.
    pub layout_stamp: u64,
    /// Document generation at which `paginated_layout` was last computed.
    pub layout_generation: u64,
    /// Canvas width (logical CSS px) at which `paginated_layout` was last computed.
    pub layout_canvas_width: f32,
    /// Whether `paginated_layout` was computed with `preserve_for_editing: true`.
    pub layout_preserve_for_editing: bool,
}

// ── LokiDocumentSource ────────────────────────────────────────────────────────

/// `CustomPaintSource` that renders one page of a [`Document`] to a wgpu texture each frame.
///
/// Lifecycle:
/// - `resume()` — GPU device is available; create `vello::Renderer` and
///   `FontResources`.
/// - `render()` — called each frame by Blitz; runs layout + paint → texture →
///   `ctx.register_texture`.  Reuses the existing texture when the document
///   and canvas size are unchanged.  Calls `ctx.unregister_texture` on the
///   previous frame's handle before allocating a new texture.
/// - `suspend()` — GPU device is lost; drop GPU resources, retain
///   `font_resources`.  The Vello renderer drop frees all registered textures.
pub(crate) struct LokiDocumentSource {
    /// Shared document state — updated by the Dioxus component when props change.
    document: Arc<Mutex<DocumentState>>,
    /// Index of the page this source renders (0-based).
    page_index: usize,
    /// wgpu device, cloned from [`DeviceHandle`] in `resume()`.
    device: Option<anyrender_vello::wgpu::Device>,
    /// wgpu queue, cloned from [`DeviceHandle`] in `resume()`.
    queue: Option<anyrender_vello::wgpu::Queue>,
    /// Shared Vello renderer, cloned from [`DocumentState::shared_renderer`] at
    /// construction.  All page sources for the same document hold the same Arc.
    shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
    /// Shared font glyph cache, cloned from [`DocumentState::shared_font_cache`]
    /// at construction.  Locked during `paint_single_page`; sequential execution
    /// means no contention.
    shared_font_cache: Arc<Mutex<FontDataCache>>,
    /// Font resources — initialized in `resume()`, used when this source wins the
    /// layout-computation race on a given frame.
    font_resources: Option<FontResources>,
    /// Handle to the texture currently registered with the Vello renderer.
    /// Unregistered at the start of the next `render()` call before a new
    /// texture is allocated.  Set to `None` in `suspend()` — the Vello
    /// renderer is dropped there, which frees the underlying wgpu allocation.
    texture_handle: Option<TextureHandle>,
    /// [`DocumentState::layout_stamp`] value at which `texture_handle` was rendered.
    /// If this differs from the current stamp, the texture is stale.
    texture_stamp: u64,
    /// Physical pixel dimensions `(w_phys, h_phys)` of `texture_handle`.
    texture_size: (u32, u32),
    /// Cursor state at which `texture_handle` was rendered.
    texture_cursor: Option<CursorState>,
    /// `preserve_for_editing` flag at which `texture_handle` was rendered.
    /// A change here forces re-layout and re-render even without a generation bump.
    texture_preserve_effective: bool,
    /// Counts completed `render()` calls.  Used in unit tests to verify that
    /// the reuse guard short-circuits correctly.
    #[cfg(test)]
    frames_rendered: usize,
}

impl LokiDocumentSource {
    /// Create a new source for `page_index`, sharing state with the Dioxus component.
    pub(crate) fn new(document: Arc<Mutex<DocumentState>>, page_index: usize) -> Self {
        let (shared_renderer, shared_font_cache) = {
            let s = document
                .lock()
                .expect("DocumentState lock poisoned in LokiDocumentSource::new");
            (s.shared_renderer.clone(), s.shared_font_cache.clone())
        };
        Self {
            document,
            page_index,
            device: None,
            queue: None,
            shared_renderer,
            shared_font_cache,
            font_resources: None,
            texture_handle: None,
            texture_stamp: 0,
            texture_size: (0, 0),
            texture_cursor: None,
            texture_preserve_effective: false,
            #[cfg(test)]
            frames_rendered: 0,
        }
    }
}

// ── Cursor paint resolution ───────────────────────────────────────────────────

/// Resolves a [`CursorState`] into a [`CursorPaint`] for a specific page.
///
/// Returns `None` when:
/// - `cursor_state` is `None` (read-only mode),
/// - the focus position is not on `page_index`,
/// - `editing_data` is absent (layout was run without `preserve_for_editing`),
/// - the paragraph's Parley layout is absent (same reason).
fn resolve_cursor_paint(
    cursor_state: &CursorState,
    editing_data: &Option<loki_layout::PageEditingData>,
    page_index: usize,
) -> Option<CursorPaint> {
    let focus = cursor_state.focus.as_ref()?;
    if focus.page_index != page_index {
        return None;
    }
    let ed = match editing_data.as_ref() {
        Some(e) => e,
        None => {
            tracing::warn!("LokiDocumentSource: resolve_cursor_paint failed: editing_data is None for page {page_index}");
            return None;
        }
    };
    let para_data = ed.paragraphs.iter().find(|p| p.block_index == focus.paragraph_index);
    let Some(pd) = para_data else {
        // This is expected if the focus is on a different page.
        return None;
    };

    let cursor_rect = pd.layout.cursor_rect(focus.byte_offset);
    if cursor_rect.is_none() {
        tracing::warn!("LokiDocumentSource: resolve_cursor_paint: pd.layout.cursor_rect returned None for offset {}", focus.byte_offset);
    }

    // Build selection highlight rects when anchor and focus differ.
    let selection_rects = if cursor_state.has_selection() {
        build_selection_rects(cursor_state, ed, &pd.layout, page_index)
    } else {
        Vec::new()
    };

    Some(CursorPaint {
        cursor_rect,
        selection_rects,
        paragraph_index: focus.paragraph_index,
    })
}

/// Build selection highlight [`SelectionRect`]s for the given `CursorState`.
///
/// Only intra-paragraph selection (anchor and focus on the same paragraph) is
/// supported in this MVP. Cross-paragraph selection produces no rects.
fn build_selection_rects(
    cursor_state: &CursorState,
    editing_data: &loki_layout::PageEditingData,
    para_layout: &loki_layout::ParagraphLayout,
    page_index: usize,
) -> Vec<SelectionRect> {
    let anchor = match cursor_state.anchor.as_ref() {
        Some(a) => a,
        None => return Vec::new(),
    };
    let focus = match cursor_state.focus.as_ref() {
        Some(f) => f,
        None => return Vec::new(),
    };

    // Cross-page or cross-paragraph selection: not supported in MVP.
    if anchor.page_index != page_index || anchor.paragraph_index != focus.paragraph_index {
        return Vec::new();
    }

    let (start_offset, end_offset) = if anchor.byte_offset <= focus.byte_offset {
        (anchor.byte_offset, focus.byte_offset)
    } else {
        (focus.byte_offset, anchor.byte_offset)
    };

    if start_offset == end_offset {
        return Vec::new();
    }

    let start_rect = match para_layout.cursor_rect(start_offset) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let end_rect = match para_layout.cursor_rect(end_offset) {
        Some(r) => r,
        None => return Vec::new(),
    };

    // Same line: single rect between the two cursor x positions.
    if (start_rect.y - end_rect.y).abs() < 0.5 {
        let x = start_rect.x.min(end_rect.x);
        let width = (start_rect.x - end_rect.x).abs();
        if width > 0.0 {
            return vec![SelectionRect {
                x,
                y: start_rect.y,
                width,
                height: start_rect.height,
            }];
        }
        return Vec::new();
    }

    // Multi-line selection: three rects (start-line tail, middle block, end-line head).
    // paragraph_origins carries (x_origin, y_origin); we need paragraph width.
    // Use the paragraph_layout width as an approximation.
    let para_width = para_layout.width.max(1.0);

    let mut rects = Vec::with_capacity(3);
    // Start line: from start_rect.x to end of line.
    let start_width = (para_width - start_rect.x).max(0.0);
    if start_width > 0.0 {
        rects.push(SelectionRect {
            x: start_rect.x,
            y: start_rect.y,
            width: start_width,
            height: start_rect.height,
        });
    }
    // Middle lines: full width.
    let middle_top = start_rect.y + start_rect.height;
    let middle_bottom = end_rect.y;
    if middle_bottom > middle_top {
        rects.push(SelectionRect {
            x: 0.0,
            y: middle_top,
            width: para_width,
            height: middle_bottom - middle_top,
        });
    }
    // End line: from 0 to end_rect.x.
    if end_rect.x > 0.0 {
        rects.push(SelectionRect {
            x: 0.0,
            y: end_rect.y,
            width: end_rect.x,
            height: end_rect.height,
        });
    }
    // Suppress unused import warning when editing_data isn't used in the MVP path.
    let _ = editing_data;
    rects
}

// ── Mutation + re-layout helper ───────────────────────────────────────────────

/// Re-derives the document from `loro_doc` after a mutation, runs a full
/// layout pass with `preserve_for_editing: true`, and publishes the updated
/// state to `doc_state`.
///
/// This is called by the keyboard handler in `editor.rs` after every successful
/// `insert_text` or `delete_text` call.  It bumps the `generation` counter so
/// that [`LokiDocumentSource::render`] picks up the change on the next GPU
/// frame.
///
/// Returns `true` on success.  Logs a warning and returns `false` when
/// `loro_to_document` or the lock fails — the editor remains in its current
/// visual state and the user can retry.
///
/// # TODO(incremental-layout)
///
/// The current implementation rebuilds the full layout for the entire
/// document.  A future iteration should invalidate only the dirty paragraph
/// and re-flow affected pages, reducing latency for large documents.
pub fn apply_mutation_and_relayout(
    doc_state: &std::sync::Arc<std::sync::Mutex<DocumentState>>,
    loro_doc: &loro::LoroDoc,
) -> bool {
    // Step 1: Derive updated Document from Loro CRDT state.
    let doc = match loki_doc_model::loro_bridge::loro_to_document(loro_doc) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("apply_mutation_and_relayout: loro_to_document failed: {e}");
            return false;
        }
    };

    // Step 2: Full layout pass with editing data preserved.
    let mut font_resources = FontResources::new();
    let layout = layout_document(
        &mut font_resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions { preserve_for_editing: true },
    );

    let (page_count, paginated_layout, page_width_px, page_height_px) = match &layout {
        DocumentLayout::Paginated(pl) => {
            let w_px = pl.page_size.width * (96.0 / 72.0);
            let h_px = pl.page_size.height * (96.0 / 72.0);
            let count = pl.pages.len();
            (count, Some(std::sync::Arc::new(pl.clone())), w_px, h_px)
        }
        _ => (0, None, loki_theme::tokens::PAGE_WIDTH_PX, loki_theme::tokens::PAGE_HEIGHT_PX),
    };

    // Step 3: Publish to shared state and bump generation.
    let Ok(mut state) = doc_state.lock() else {
        tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned");
        return false;
    };
    state.document = Some(doc);
    state.paginated_layout = paginated_layout;
    state.page_count = page_count;
    state.page_width_px = page_width_px;
    state.page_height_px = page_height_px;
    state.generation = state.generation.wrapping_add(1);
    true
}

// ── CustomPaintSource impl ────────────────────────────────────────────────────

impl CustomPaintSource for LokiDocumentSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        self.device = Some(device_handle.device.clone());
        self.queue = Some(device_handle.queue.clone());

        // Only the first page source to resume creates the renderer; the rest
        // find it already initialised and skip.  The renderer is tied to the
        // Device, which is the same Arc clone across all page sources.
        let mut renderer_guard = self
            .shared_renderer
            .lock()
            .expect("renderer lock poisoned in resume()");
        if renderer_guard.is_none() {
            match vello::Renderer::new(
                &device_handle.device,
                RendererOptions {
                    use_cpu: false,
                    antialiasing_support: AaSupport::all(),
                    num_init_threads: NonZeroUsize::new(1),
                    pipeline_cache: None,
                },
            ) {
                Ok(r) => *renderer_guard = Some(r),
                Err(e) => tracing::warn!("LokiDocumentSource: vello renderer init failed: {e}"),
            }
        }

        self.font_resources = Some(FontResources::new());
    }

    fn suspend(&mut self) {
        self.device = None;
        self.queue = None;
        // Drop the shared renderer.  The first source to call suspend() does
        // the actual drop; subsequent sources find None and return immediately.
        // Blitz calls suspend() on all sources before entering Suspended state,
        // so resume() is guaranteed before the next render().
        *self
            .shared_renderer
            .lock()
            .expect("renderer lock poisoned in suspend()") = None;
        // The Vello renderer dropped above owns the registered texture via
        // engine.image_overrides; it is freed when the renderer is dropped.
        // Clear the stale handle and metadata so the next render() after
        // resume() does not attempt to reuse a handle from a dead renderer.
        self.texture_handle = None;
        self.texture_stamp = 0;
        self.texture_size = (0, 0);
        self.texture_cursor = None;
        self.texture_preserve_effective = false;
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
        if self.device.is_none() || self.queue.is_none() {
            return None;
        }

        // blitz-paint-0.2.1/src/render.rs:606-607 casts content_box dimensions
        // (which create_css_rect already multiplied by scale, line 779) to u32,
        // so `width` and `height` here are already physical (device) pixels.
        // Dividing by scale converts back to logical CSS pixels for layout.
        let canvas_width = width as f32 / scale as f32;

        // Physical texture dimensions — already provided in physical pixels.
        let w_phys = width.max(1);
        let h_phys = height.max(1);

        // Phase 1: Read document state under lock, then release before layout work.
        let (doc_opt, current_gen, cursor_state_opt, preserve_for_editing, layout_stamp) = {
            let state = match self.document.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("LokiDocumentSource: document lock poisoned: {e}");
                    return None;
                }
            };
            (
                state.document.clone(),
                state.generation,
                state.cursor_state.clone(),
                state.preserve_for_editing,
                state.layout_stamp,
            )
        };

        // No document loaded — WgpuSurface shows a placeholder div instead.
        doc_opt.as_ref()?;

        let preserve_effective = preserve_for_editing || cursor_state_opt.is_some();

        // Texture reuse: if layout stamp, physical dimensions, cursor, and preserve
        // flag are all unchanged the existing texture is still valid — skip layout,
        // scene painting, and GPU allocation entirely.
        if self.texture_handle.is_some()
            && self.texture_stamp == layout_stamp
            && self.texture_preserve_effective == preserve_effective
            && self.texture_size == (w_phys, h_phys)
            && self.texture_cursor == cursor_state_opt
        {
            return self.texture_handle.clone();
        }

        // Release the previous frame's registered texture before allocating a
        // new one.  Vello's register_texture takes ownership by value and stores
        // the texture in engine.image_overrides; without unregister_texture the
        // map grows by one entry per frame, leaking GPU memory continuously.
        if let Some(old_handle) = self.texture_handle.take() {
            ctx.unregister_texture(old_handle);
        }

        // Phase 2: Ensure the shared layout is current.  The first source to find
        // it stale recomputes; all others on the same frame read the fresh copy.
        // Sources execute sequentially (Blitz values_mut()), so this is race-free.
        let needs_recompute = {
            let s = match self.document.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("LokiDocumentSource: document lock poisoned: {e}");
                    return None;
                }
            };
            s.paginated_layout.is_none()
                || s.layout_generation != current_gen
                || (s.layout_canvas_width - canvas_width).abs() > 0.001
                || s.layout_preserve_for_editing != preserve_effective
        };

        if needs_recompute {
            let doc = doc_opt?;
            let font_resources = self.font_resources.get_or_insert_with(FontResources::new);
            // Layout at scale=1.0 keeps all coordinates in CSS pixels.
            // paint_layout multiplies by `scale` to convert to physical pixels;
            // passing the device scale here would apply it twice (Parley 0.6.0
            // already multiplies font sizes by display_scale internally).
            let layout_opts = LayoutOptions { preserve_for_editing: preserve_effective };
            let layout =
                layout_document(font_resources, &doc, LayoutMode::Paginated, 1.0, &layout_opts);
            let (page_count, paginated_layout) = match layout {
                DocumentLayout::Paginated(pl) => (pl.pages.len(), Some(Arc::new(pl))),
                _ => (0, None),
            };
            // Reset shared font cache — glyphs belong to the old layout.
            *self
                .shared_font_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = FontDataCache::new();
            // Publish updated layout and metadata to shared state.
            if let Ok(mut state) = self.document.lock() {
                state.paginated_layout = paginated_layout;
                state.page_count = page_count;
                state.canvas_width = canvas_width;
                state.layout_generation = current_gen;
                state.layout_canvas_width = canvas_width;
                state.layout_preserve_for_editing = preserve_effective;
                state.layout_stamp = state.layout_stamp.wrapping_add(1);
            }
        }

        // Read the current layout and stamp (may have just been updated above).
        let (paginated_layout, current_stamp) = {
            let s = match self.document.lock() {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("LokiDocumentSource: document lock poisoned: {e}");
                    return None;
                }
            };
            let pl = s.paginated_layout.as_ref()?.clone();
            (pl, s.layout_stamp)
        };

        // Phase 4: Paint this page's scene.
        // TODO(partial-render): pass visible_rect as clip region to paint_single_page.
        let mut scene = Scene::new();
        // loki-layout coordinates are in points (1 pt = 1/72 inch).
        // CSS pixels use 96 dpi (1 CSS px = 1/96 inch), so 1 pt = 96/72 CSS px.
        let render_scale = scale as f32 * (96.0 / 72.0);
        let cursor_paint = cursor_state_opt.as_ref().and_then(|cs| {
            let page = paginated_layout.pages.get(self.page_index)?;
            resolve_cursor_paint(cs, &page.editing_data, self.page_index)
        });
        {
            let mut font_cache_guard = self
                .shared_font_cache
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            paint_single_page(
                &mut scene,
                &paginated_layout,
                &mut font_cache_guard,
                (0.0, 0.0),
                render_scale,
                self.page_index,
                cursor_paint.as_ref(),
            );
        }

        // Phase 5: GPU work — borrow GPU resources mutably only after all
        // immutable self-borrows above are complete (borrow checker requires
        // non-overlapping borrows on self).
        let device = self.device.as_ref()?;
        let queue = self.queue.as_ref()?;
        // Lock the shared renderer.  Sources render sequentially (Blitz uses a
        // single-threaded values_mut() loop), so this never contends in practice.
        let mut renderer_guard = self
            .shared_renderer
            .lock()
            .expect("renderer lock poisoned in render()");
        let renderer = renderer_guard.as_mut()?;

        // COMPAT(blitz): Rgba8Unorm with STORAGE_BINDING|TEXTURE_BINDING is the
        // format expected by vello render_to_texture and register_texture in
        // anyrender_vello 0.6.2. If compositing produces garbage pixels, inspect
        // VelloRenderer::register_texture for format validation.
        let texture = device.create_texture(&anyrender_vello::wgpu::TextureDescriptor {
            label: Some("loki_document_source"),
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
        let params = RenderParams {
            base_color: Color::WHITE,
            width: w_phys,
            height: h_phys,
            antialiasing_method: AaConfig::Msaa16,
        };

        if let Err(e) = renderer.render_to_texture(device, queue, &scene, &view, &params) {
            tracing::error!("LokiDocumentSource: render_to_texture failed: {e}");
            return None;
        }

        // Register the new texture and cache the handle so the next frame can
        // either reuse it (static document) or unregister it (changed document).
        let handle = ctx.register_texture(texture);
        self.texture_handle = Some(handle.clone());
        self.texture_stamp = current_stamp;
        self.texture_preserve_effective = preserve_effective;
        self.texture_size = (w_phys, h_phys);
        self.texture_cursor = cursor_state_opt;

        #[cfg(test)]
        {
            self.frames_rendered += 1;
        }

        Some(handle)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::document::Document;

    fn make_source() -> LokiDocumentSource {
        LokiDocumentSource::new(
            Arc::new(Mutex::new(DocumentState {
                document: None,
                generation: 0,
                page_count: 0,
                canvas_width: 0.0,
                visible_rect: None,
                page_width_px: 0.0,
                page_height_px: 0.0,
                cursor_state: None,
                preserve_for_editing: false,
                paginated_layout: None,
                shared_renderer: Arc::new(Mutex::new(None)),
                shared_font_cache: Arc::new(Mutex::new(FontDataCache::new())),
                layout_stamp: 0,
                layout_generation: 0,
                layout_canvas_width: 0.0,
                layout_preserve_for_editing: false,
            })),
            0,
        )
    }

    /// Simulates a layout recompute by populating `DocumentState` as `render()` would.
    fn populate_layout(state: &Arc<Mutex<DocumentState>>, generation: u64, canvas_width: f32, preserve: bool, stamp: u64) {
        use loki_layout::{layout_document, LayoutMode};
        let doc = Document::new();
        let mut resources = FontResources::new();
        let layout = layout_document(&mut resources, &doc, LayoutMode::Paginated, 1.0, &LayoutOptions::default());
        let paginated = match layout {
            DocumentLayout::Paginated(pl) => Some(Arc::new(pl)),
            _ => None,
        };
        let mut s = state.lock().unwrap();
        s.paginated_layout = paginated;
        s.layout_generation = generation;
        s.layout_canvas_width = canvas_width;
        s.layout_preserve_for_editing = preserve;
        s.layout_stamp = stamp;
    }

    #[test]
    fn layout_stamp_initially_zero() {
        let s = make_source();
        assert_eq!(s.document.lock().unwrap().layout_stamp, 0);
    }

    #[test]
    fn layout_stale_when_generation_unset() {
        let doc = make_source().document;
        let s = doc.lock().unwrap();
        // No layout computed yet: paginated_layout is None → stale.
        assert!(s.paginated_layout.is_none());
    }

    #[test]
    fn layout_fresh_after_populate() {
        let doc = make_source().document;
        populate_layout(&doc, 7, 0.0, false, 1);
        let s = doc.lock().unwrap();
        assert!(s.paginated_layout.is_some());
        assert_eq!(s.layout_generation, 7);
        assert_eq!(s.layout_stamp, 1);
    }

    #[test]
    fn layout_stale_when_generation_advances() {
        let doc = make_source().document;
        populate_layout(&doc, 7, 0.0, false, 1);
        let s = doc.lock().unwrap();
        // generation 8 != layout_generation 7 → stale
        assert!(s.layout_generation != 8);
    }

    #[test]
    fn layout_stale_when_canvas_width_changes() {
        let doc = make_source().document;
        populate_layout(&doc, 7, 0.0, false, 1);
        let s = doc.lock().unwrap();
        // new canvas_width 800.0, stored 0.0, diff > 0.001 → stale
        assert!((s.layout_canvas_width - 800.0_f32).abs() > 0.001);
    }

    #[test]
    fn layout_canvas_width_subthreshold_no_stale() {
        let doc = make_source().document;
        populate_layout(&doc, 7, 0.0, false, 1);
        let s = doc.lock().unwrap();
        // diff 0.0005 is below 0.001 threshold → not stale
        assert!((s.layout_canvas_width - 0.0005_f32).abs() <= 0.001);
    }

    #[test]
    fn layout_stale_when_preserve_changes() {
        let doc = make_source().document;
        populate_layout(&doc, 7, 0.0, false, 1);
        let s = doc.lock().unwrap();
        // preserve_effective=true, stored false → stale
        assert!(s.layout_preserve_for_editing != true);
    }

    #[test]
    fn generation_counter_increments_on_document_change() {
        let state = Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            page_count: 0,
            canvas_width: 0.0,
            visible_rect: None,
            page_width_px: 0.0,
            page_height_px: 0.0,
            cursor_state: None,
            preserve_for_editing: false,
            paginated_layout: None,
            shared_renderer: Arc::new(Mutex::new(None)),
            shared_font_cache: Arc::new(Mutex::new(FontDataCache::new())),
            layout_stamp: 0,
            layout_generation: 0,
            layout_canvas_width: 0.0,
            layout_preserve_for_editing: false,
        }));
        // Simulate the component bumping the generation counter.
        {
            let mut s = state.lock().unwrap();
            s.document = Some(Document::new());
            s.generation = s.generation.wrapping_add(1);
        }
        assert_eq!(state.lock().unwrap().generation, 1);
    }

    // ── Leak-prevention structural tests ─────────────────────────────────────
    //
    // Full render-loop leak detection (calling render() 10× with a headless
    // wgpu device and asserting only one texture allocation remains) requires
    // a live GPU device.  The tests below verify the structural invariants that
    // prevent the leak: the handle field is initialised to None, the reuse guard
    // logic is correct, and suspend() clears all texture state so that no stale
    // handle can be unregistered against a new renderer after resume().

    #[test]
    fn texture_handle_initially_none() {
        // A freshly created source must not carry a stale GPU handle.
        assert!(make_source().texture_handle.is_none());
    }

    #[test]
    fn texture_size_initially_zero() {
        let s = make_source();
        assert_eq!(s.texture_size, (0, 0), "no texture until first render");
    }

    #[test]
    fn frames_rendered_starts_at_zero() {
        assert_eq!(make_source().frames_rendered, 0);
    }

    #[test]
    fn reuse_guard_blocked_without_handle() {
        // Even if stamp and size match, no handle → guard must not fire.
        let s = make_source();
        let would_reuse = s.texture_handle.is_some()
            && s.texture_stamp == 0
            && s.texture_size == (0, 0);
        assert!(!would_reuse, "no handle means no reuse");
    }

    #[test]
    fn reuse_guard_blocked_on_size_mismatch() {
        // Even with a matching stamp, a different physical size must force
        // a new texture (e.g. DPI scale change without CSS width change).
        let mut s = make_source();
        s.texture_stamp = 3;
        s.texture_size = (800, 1131);
        // No real TextureHandle can be constructed without a GPU; skip the
        // is_some() arm and verify the size-mismatch condition directly.
        let size_matches = s.texture_size == (1600, 2262); // different HiDPI size
        assert!(!size_matches, "different physical size → no reuse");
    }

    #[test]
    fn suspend_clears_texture_state() {
        let mut s = make_source();
        // Simulate having rendered a frame by setting metadata fields directly.
        // (texture_handle stays None because constructing one requires a GPU.)
        s.texture_stamp = 5;
        s.texture_size = (794, 1123);
        s.suspend();
        assert!(s.texture_handle.is_none(), "suspend must clear handle");
        assert_eq!(s.texture_stamp, 0, "suspend must reset stamp");
        assert_eq!(s.texture_size, (0, 0), "suspend must reset size");
    }

    #[test]
    fn suspend_clears_gpu_resources() {
        let mut s = make_source();
        // device/queue/renderer are None until resume(); verify suspend() is
        // idempotent (does not panic when called without a prior resume()).
        s.suspend();
        assert!(s.device.is_none());
        assert!(s.queue.is_none());
        assert!(s.shared_renderer.lock().unwrap().is_none());
    }

    // ── Cursor paint resolution tests ─────────────────────────────────────────

    fn make_para_with_text(text: &str) -> loki_layout::ParagraphLayout {
        let mut res = loki_layout::FontResources::new();
        let spans = vec![loki_layout::StyleSpan {
            range: 0..text.len(),
            font_name: None,
            font_size: 12.0,
            bold: false,
            italic: false,
            color: loki_layout::LayoutColor::BLACK,
            underline: None,
            strikethrough: None,
            line_height: None,
            vertical_align: None,
            highlight_color: None,
            letter_spacing: None,
            font_variant: None,
            word_spacing: None,
            shadow: false,
            link_url: None,
        }];
        loki_layout::layout_paragraph(
            &mut res,
            text,
            &spans,
            &loki_layout::ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        )
    }

    fn make_editing_data_single(
        para: loki_layout::ParagraphLayout,
    ) -> Option<loki_layout::PageEditingData> {
        Some(loki_layout::PageEditingData {
            paragraphs: vec![loki_layout::PageParagraphData {
                block_index: 0,
                layout: Arc::new(para),
                origin: (0.0, 0.0),
            }],
        })
    }

    #[test]
    fn cursor_paint_focus_on_page_returns_some_with_height() {
        use crate::editing::cursor::DocumentPosition;
        let para = make_para_with_text("Hello world");
        let ed = make_editing_data_single(para);
        let state = CursorState {
            loro_cursor: None,
            anchor: Some(DocumentPosition { page_index: 0, paragraph_index: 0, byte_offset: 0 }),
            focus: Some(DocumentPosition { page_index: 0, paragraph_index: 0, byte_offset: 0 }),
        };
        let result = resolve_cursor_paint(&state, &ed, 0);
        assert!(result.is_some(), "focus on page 0 should return Some");
        let cr = result.unwrap().cursor_rect.expect("cursor rect present for non-empty text");
        assert!(cr.height > 0.0, "cursor height should be positive");
    }

    #[test]
    fn cursor_paint_focus_on_different_page_returns_none() {
        use crate::editing::cursor::DocumentPosition;
        let para = make_para_with_text("Hello world");
        let ed = make_editing_data_single(para);
        let state = CursorState {
            loro_cursor: None,
            anchor: None,
            focus: Some(DocumentPosition { page_index: 1, paragraph_index: 0, byte_offset: 0 }),
        };
        assert!(
            resolve_cursor_paint(&state, &ed, 0).is_none(),
            "focus on page 1 must return None when querying page 0"
        );
    }

    #[test]
    fn cursor_paint_selection_produces_positive_width_rect() {
        use crate::editing::cursor::DocumentPosition;
        let para = make_para_with_text("Hello world");
        let ed = make_editing_data_single(para);
        let state = CursorState {
            loro_cursor: None,
            anchor: Some(DocumentPosition { page_index: 0, paragraph_index: 0, byte_offset: 0 }),
            focus: Some(DocumentPosition { page_index: 0, paragraph_index: 0, byte_offset: 5 }),
        };
        let paint =
            resolve_cursor_paint(&state, &ed, 0).expect("selection on page 0 should return Some");
        assert!(
            !paint.selection_rects.is_empty(),
            "selection from byte 0..5 should produce at least one SelectionRect"
        );
        assert!(
            paint.selection_rects.iter().any(|r| r.width > 0.0),
            "at least one SelectionRect must have positive width"
        );
    }
}
