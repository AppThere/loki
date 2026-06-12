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
};

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
    let layout = {
        let Ok(state) = doc_state.lock() else { return };
        let fr_arc = state.shared_font_resources.clone();
        drop(state);
        let mut fr = fr_arc.lock().unwrap_or_else(|e| e.into_inner());
        // New document: drop the previous document's memoised paragraph layouts
        // so the shaping cache does not accumulate across loads.
        fr.clear_paragraph_cache();
        loki_layout::layout_document(
            &mut fr,
            doc,
            LayoutMode::Paginated,
            1.0,
            &LayoutOptions {
                preserve_for_editing: true,
            },
        )
    };
    let (page_count, paginated_layout, page_width_px, page_height_px) = match &layout {
        DocumentLayout::Paginated(pl) => {
            let w = pl.page_size.width * (96.0 / 72.0);
            let h = pl.page_size.height * (96.0 / 72.0);
            (pl.pages.len(), Some(Arc::new(pl.clone())), w, h)
        }
        _ => (
            0,
            None,
            appthere_ui::tokens::PAGE_WIDTH_PX,
            appthere_ui::tokens::PAGE_HEIGHT_PX,
        ),
    };
    let Ok(mut state) = doc_state.lock() else {
        return;
    };
    state.document = Some(Arc::new(doc.clone()));
    state.paginated_layout = paginated_layout;
    state.page_count = page_count;
    state.page_width_px = page_width_px;
    state.page_height_px = page_height_px;
    // A new document is being seeded — discard any incremental reader bound to
    // the previous Loro document so it re-seeds against the new one.
    state.incremental = None;
    state.generation = state.generation.wrapping_add(1);
}

/// Re-derives the document from `loro_doc`, runs a full layout pass, and
/// publishes the updated state to `doc_state`.
///
/// Call after any `insert_text` / `delete_text` / formatting mutation.
/// Returns `true` on success.
pub fn apply_mutation_and_relayout(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: &loro::LoroDoc,
) -> bool {
    // Step 1+2: Derive the Document from Loro — incrementally re-deriving only
    // the changed block(s) when possible — and restore the style catalog and
    // source from the previously published document (neither is stored in Loro).
    let doc = {
        let Ok(mut state) = doc_state.lock() else {
            tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (derive)");
            return false;
        };
        // Lazily seed the incremental reader against this Loro document.
        if state.incremental.is_none() {
            match IncrementalReader::seed(loro_doc) {
                Ok(reader) => state.incremental = Some(reader),
                Err(e) => {
                    tracing::warn!("apply_mutation_and_relayout: incremental seed failed: {e}");
                    return false;
                }
            }
        }
        let mut doc = match state.incremental.as_mut() {
            Some(reader) => match reader.update(loro_doc) {
                Ok(d) => d.clone(),
                Err(e) => {
                    tracing::warn!("apply_mutation_and_relayout: incremental update failed: {e}");
                    return false;
                }
            },
            None => return false,
        };
        if let Some(orig) = &state.document {
            doc.styles = orig.styles.clone();
            doc.source = orig.source.clone();
        }
        doc
    };

    // Step 3: Full layout pass with editing data preserved.
    let layout = {
        let Ok(state) = doc_state.lock() else {
            tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (font)");
            return false;
        };
        let fr_arc = state.shared_font_resources.clone();
        drop(state);
        let mut fr = fr_arc.lock().unwrap_or_else(|e| e.into_inner());
        loki_layout::layout_document(
            &mut fr,
            &doc,
            LayoutMode::Paginated,
            1.0,
            &LayoutOptions {
                preserve_for_editing: true,
            },
        )
    };

    let (page_count, paginated_layout, page_width_px, page_height_px) = match &layout {
        DocumentLayout::Paginated(pl) => {
            let w = pl.page_size.width * (96.0 / 72.0);
            let h = pl.page_size.height * (96.0 / 72.0);
            (pl.pages.len(), Some(Arc::new(pl.clone())), w, h)
        }
        _ => (
            0,
            None,
            appthere_ui::tokens::PAGE_WIDTH_PX,
            appthere_ui::tokens::PAGE_HEIGHT_PX,
        ),
    };

    // Step 4: Publish.
    let Ok(mut state) = doc_state.lock() else {
        tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (publish)");
        return false;
    };
    state.document = Some(Arc::new(doc));
    state.paginated_layout = paginated_layout;
    state.page_count = page_count;
    state.page_width_px = page_width_px;
    state.page_height_px = page_height_px;
    state.generation = state.generation.wrapping_add(1);
    true
}
