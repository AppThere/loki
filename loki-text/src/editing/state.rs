// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared document editing state for the Loki text editor.
//!
//! [`DocumentState`] holds the data needed by editing-layer event handlers
//! (cursor hit-testing, page count, mutation context).  GPU rendering state
//! has been moved to `loki_renderer::RendererState`.

use std::sync::{Arc, Mutex};

use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::IncrementalReader;
use loki_layout::{
    ContinuousLayout, DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout,
    PaginatedReuse,
};

use super::relayout::{LaidOut, page_metrics, relayout_paginated};

#[path = "state_apply.rs"]
mod apply;
pub use apply::apply_mutation_and_relayout;

/// Shared document editing state.
///
/// Owned by `EditorInner` behind an `Arc<Mutex<DocumentState>>` and shared
/// with event handlers for cursor hit-testing and mutation tracking.
pub struct DocumentState {
    /// Currently loaded document, or `None` when no file is open.
    /// Stored as `Arc` so post-mutation versions can be passed cheaply to
    /// the renderer without cloning the full document tree.
    pub document: Option<Arc<Document>>,
    /// Bumped after each document mutation; drives cursor re-render and
    /// `layout_document` invalidation.
    pub generation: u64,
    /// Number of pages in the current paginated layout; 0 when not loaded.
    pub page_count: usize,
    /// Most recently computed paginated layout for hit-testing mouse/touch
    /// events.  `None` until the first load or mutation.
    pub paginated_layout: Option<Arc<PaginatedLayout>>,
    /// Page width in CSS px from the current layout (A4 fallback: 794 px).
    pub page_width_px: f32,
    /// Page height in CSS px from the current layout (A4 fallback: 1123 px).
    pub page_height_px: f32,
    /// Shared Parley font + shaping context — one per editor to avoid the
    /// ≈20 MB font-scan cost on every mutation.
    pub shared_font_resources: Arc<Mutex<FontResources>>,
    /// Lazily-computed reflow layout for reflow-mode navigation, keyed by
    /// `(generation, content-width key)`.  Recomputed when stale.  Separate from
    /// the renderer's copy; only built when the user navigates in reflow mode.
    pub reflow_cache: Option<(u64, i32, Arc<ContinuousLayout>)>,
    /// Incremental Loro→Document reconstructor. Lazily seeded on the first
    /// mutation and reset (`None`) when a new document is loaded, so a keystroke
    /// re-derives only the changed block instead of the whole document.
    pub incremental: Option<IncrementalReader>,
    /// Reuse metadata (clean-page-top checkpoints) for the current paginated
    /// layout, enabling `loki_layout::relayout_paginated_incremental` to reuse
    /// unchanged pages on the next edit. `None` until the first layout.
    pub layout_reuse: Option<PaginatedReuse>,
}

impl DocumentState {
    /// Creates a fresh state with no document loaded.
    pub fn new() -> Self {
        Self {
            document: None,
            generation: 0,
            page_count: 0,
            paginated_layout: None,
            page_width_px: appthere_ui::tokens::PAGE_WIDTH_PX,
            page_height_px: appthere_ui::tokens::PAGE_HEIGHT_PX,
            shared_font_resources: Arc::new(Mutex::new(FontResources::new())),
            reflow_cache: None,
            incremental: None,
            layout_reuse: None,
        }
    }
}

/// Returns the reflow [`ContinuousLayout`] for the current document laid out at
/// `content_width_pt`, computing and caching it on `DocumentState` when stale.
///
/// Used by reflow-mode arrow navigation, which needs the reflowed line geometry
/// (the paginated layout wraps at a different width). Returns `None` when no
/// document is loaded.
pub fn ensure_reflow_layout(
    doc_state: &Arc<Mutex<DocumentState>>,
    content_width_pt: f32,
) -> Option<Arc<ContinuousLayout>> {
    let mut state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
    let doc = state.document.clone()?;
    let width_key = content_width_pt.round() as i32;
    if let Some((cached_gen, key, layout)) = &state.reflow_cache
        && *cached_gen == state.generation
        && *key == width_key
    {
        return Some(layout.clone());
    }
    let layout = {
        let mut resources = state
            .shared_font_resources
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let options = LayoutOptions {
            preserve_for_editing: true,
            spell: crate::editing::spell::active(),
            ..Default::default()
        };
        match loki_layout::layout_document(
            &mut resources,
            &doc,
            LayoutMode::Reflow {
                available_width: content_width_pt,
            },
            1.0,
            &options,
        ) {
            DocumentLayout::Continuous(cl) => Arc::new(cl),
            _ => return None,
        }
    };
    state.reflow_cache = Some((state.generation, width_key, layout.clone()));
    Some(layout)
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::new()
    }
}

/// Seeds the layout from an already-loaded [`Document`] without going through
/// Loro.  Call this once after the document is first loaded so that
/// [`make_mousedown_handler`] finds a populated `paginated_layout` on the
/// very first click.
///
/// [`make_mousedown_handler`]: crate::routes::editor::editor_pointer::make_mousedown_handler
pub fn seed_layout_from_document(doc_state: &Arc<Mutex<DocumentState>>, doc: &Document) {
    // Open-path timing under the `loki_text::open` target (measurable on-device
    // via `RUST_LOG=loki_text::open=info`). This sync entry point is retained
    // for tests/tools; the editor opens via the off-thread path instead.
    let started = std::time::Instant::now();
    let fr_arc = {
        let Ok(state) = doc_state.lock() else { return };
        state.shared_font_resources.clone()
    };
    let layout = compute_seed_layout(&fr_arc, doc);
    let page_count = publish_seed_layout(doc_state, doc, layout);
    tracing::info!(
        target: "loki_text::open",
        pages = page_count,
        elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
        "seed_layout_from_document: first paginated layout complete",
    );
}

/// Computes the first paginated layout for `doc` using `font_resources`.
///
/// Only locks the font resources (not [`DocumentState`]), so it can run on a
/// worker thread; publish the result on the main thread with
/// [`publish_seed_layout`]. Both [`FontResources`] and the returned layout are
/// `Send`, which is what makes the off-main-thread open path possible.
pub(crate) fn compute_seed_layout(
    font_resources: &Arc<Mutex<FontResources>>,
    doc: &Document,
) -> LaidOut {
    // Open-path timing (the worker thread). Logged under `loki_text::open` so the
    // CPU layout cost of opening a document is visible on-device:
    //   RUST_LOG=loki_text::open=info cargo run -p loki-text --release
    let started = std::time::Instant::now();
    let mut fr = font_resources.lock().unwrap_or_else(|e| e.into_inner());
    let lock_ms = started.elapsed().as_secs_f64() * 1000.0;
    // New document: drop the previous document's memoised paragraph layouts so
    // the shaping cache does not accumulate across loads.
    fr.clear_paragraph_cache();
    let out = relayout_paginated(&mut fr, doc, None);
    tracing::info!(
        target: "loki_text::open",
        pages = out.layout.pages.len(),
        font_lock_ms = lock_ms,
        elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
        "compute_seed_layout: worker paginated layout complete",
    );
    out
}

/// Publishes a pre-computed paginated layout into `doc_state`, returning the
/// page count. Pairs with [`compute_seed_layout`] for the off-thread open path.
pub(crate) fn publish_seed_layout(
    doc_state: &Arc<Mutex<DocumentState>>,
    doc: &Document,
    laid_out: LaidOut,
) -> usize {
    let (page_count, page_width_px, page_height_px) = page_metrics(&laid_out.layout);
    let Ok(mut state) = doc_state.lock() else {
        return 0;
    };
    state.document = Some(Arc::new(doc.clone()));
    state.paginated_layout = Some(Arc::new(laid_out.layout));
    state.layout_reuse = Some(laid_out.reuse);
    state.page_count = page_count;
    state.page_width_px = page_width_px;
    state.page_height_px = page_height_px;
    // A new document is being seeded — discard any incremental reader bound to
    // the previous Loro document so it re-seeds against the new one.
    state.incremental = None;
    state.generation = state.generation.wrapping_add(1);
    page_count
}
