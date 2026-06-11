// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
//! `Option<(u64, RenderLayout)>`.  The guard keeps the layout allocation
//! alive without cloning.  If the stored generation differs from the requested
//! generation the layout is recomputed under the same lock acquisition, so the
//! check and the write are atomic with respect to concurrent readers.
//!
//! # Render modes
//!
//! The source lays the document out either as print-fidelity pages
//! ([`RenderMode::Paginated`]) or as a continuous web-style flow at a caller
//! supplied width ([`RenderMode::Reflow`]), presented as virtual tiles — see
//! [`crate::render_layout`].  Switching modes invalidates the cache and
//! advances the generation so every tile re-renders.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use loki_doc_model::document::Document;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions};
use loki_vello::FontDataCache;

use crate::render_layout::{MIN_REFLOW_CONTENT_PT, REFLOW_PADDING_PT, RenderLayout, RenderMode};

// ── A4 page size at 96 dpi ────────────────────────────────────────────────────

/// Default page width in pixels at 96 dpi (A4: 210 mm → ~794 px).
pub(crate) const A4_WIDTH_PX: u32 = 794;
/// Default page height in pixels at 96 dpi (A4: 297 mm → ~1123 px).
pub(crate) const A4_HEIGHT_PX: u32 = 1123;

// ── DocPageSource ─────────────────────────────────────────────────────────────

/// Bridges `loki-doc-model` to the [`appthere_canvas::PageSource`] trait and
/// `LokiPageSource`.
///
/// Holds a shared reference to the document, a generation counter, and a
/// generation-keyed layout cache.  Multiple [`LokiPageSource`] instances share
/// one `DocPageSource` via [`Arc`]; whichever page renders first after a
/// generation advance causes the layout recompute; the rest reuse the result.
pub struct DocPageSource {
    /// Current document — interior-mutable so callers can push post-mutation
    /// documents without recreating the source.
    doc: Mutex<Arc<Document>>,
    /// Generation-keyed layout cache.  `None` until first render.
    layout_cache: Mutex<Option<(u64, RenderLayout)>>,
    /// Active render mode.  Changing it invalidates the layout cache.
    render_mode: Mutex<RenderMode>,
    /// Shared font cache for rendering (used by the `PageSource::render` path).
    pub(crate) font_cache: Mutex<FontDataCache>,
    /// Lazily-initialised Vello renderer for the `PageSource::render` path.
    pub(crate) renderer: Mutex<Option<vello::Renderer>>,
    /// Monotone generation counter.  Starts at 1 so that `LokiPageSource`
    /// (whose `texture_generation` initialises to 0) always renders on its
    /// first frame.
    generation: Arc<AtomicU64>,
}

impl DocPageSource {
    /// Creates a new [`DocPageSource`] backed by `doc`.
    pub fn new(doc: Arc<Document>) -> Self {
        Self {
            doc: Mutex::new(doc),
            layout_cache: Mutex::new(None),
            render_mode: Mutex::new(RenderMode::Paginated),
            font_cache: Mutex::new(FontDataCache::new()),
            renderer: Mutex::new(None),
            generation: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Returns the current document.
    pub fn document(&self) -> Arc<Document> {
        self.doc.lock().unwrap_or_else(|e| e.into_inner()).clone()
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

    /// Switches the render mode (paginated ⇆ reflow / reflow width change).
    ///
    /// No-op when the mode is unchanged (reflow widths within 0.5 pt).  On a
    /// real change the layout cache is cleared and the generation advanced so
    /// every tile re-renders against the new layout.
    pub fn set_render_mode(&self, mode: RenderMode) {
        let mut guard = self.render_mode.lock().unwrap_or_else(|e| e.into_inner());
        if guard.matches(&mode) {
            return;
        }
        *guard = mode;
        drop(guard);
        *self.layout_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Replaces the document and invalidates the layout cache.
    ///
    /// Compares by [`Arc`] pointer; returns immediately when the pointer is
    /// unchanged (no allocation cost between renders with no mutations).
    /// When the doc has changed, clears the layout cache and advances the
    /// generation so the next [`Self::layout_for_generation`] call recomputes.
    pub fn update_doc(&self, new_doc: Arc<Document>) {
        let mut guard = self.doc.lock().unwrap_or_else(|e| e.into_inner());
        if Arc::ptr_eq(&*guard, &new_doc) {
            return;
        }
        *guard = new_doc;
        drop(guard);
        *self.layout_cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Returns a guard holding the layout for `generation`, recomputing if stale.
    ///
    /// The guard keeps the [`RenderLayout`] alive without cloning.
    /// Callers extract `&RenderLayout` via:
    /// ```ignore
    /// let guard = source.layout_for_generation(doc_gen);
    /// let Some((_, layout)) = guard.as_ref() else { return; };
    /// ```
    pub fn layout_for_generation(
        &self,
        generation: u64,
    ) -> MutexGuard<'_, Option<(u64, RenderLayout)>> {
        let mut guard = self.layout_cache.lock().unwrap_or_else(|e| e.into_inner());
        let needs_recompute = guard
            .as_ref()
            .map(|(g, _)| *g != generation)
            .unwrap_or(true);
        if needs_recompute {
            let doc = self.doc.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let mode = *self.render_mode.lock().unwrap_or_else(|e| e.into_inner());
            let mut resources = FontResources::new();
            let layout = match mode {
                RenderMode::Paginated => {
                    let options = LayoutOptions {
                        preserve_for_editing: true,
                    };
                    match loki_layout::layout_document(
                        &mut resources,
                        &doc,
                        LayoutMode::Paginated,
                        1.0,
                        &options,
                    ) {
                        DocumentLayout::Paginated(pl) => RenderLayout::Paginated(pl),
                        _ => unreachable!(
                            "LayoutMode::Paginated must return DocumentLayout::Paginated"
                        ),
                    }
                }
                RenderMode::Reflow { available_width_pt } => {
                    // Continuous layouts carry no editing data, so skip the
                    // preserve-for-editing bookkeeping.
                    let options = LayoutOptions {
                        preserve_for_editing: false,
                    };
                    let content_width =
                        (available_width_pt - 2.0 * REFLOW_PADDING_PT).max(MIN_REFLOW_CONTENT_PT);
                    match loki_layout::layout_document(
                        &mut resources,
                        &doc,
                        LayoutMode::Reflow {
                            available_width: content_width,
                        },
                        1.0,
                        &options,
                    ) {
                        DocumentLayout::Continuous(cl) => RenderLayout::Reflow(cl),
                        _ => unreachable!(
                            "LayoutMode::Reflow must return DocumentLayout::Continuous"
                        ),
                    }
                }
            };
            *guard = Some((generation, layout));
        }
        guard
    }
}
