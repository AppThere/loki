// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared document editing state for the Loki text editor.
//!
//! [`DocumentState`] holds the data needed by editing-layer event handlers
//! (cursor hit-testing, page count, mutation context).  GPU rendering state
//! has been moved to `loki_renderer::RendererState`.

use std::sync::{Arc, Mutex};

use loki_doc_model::document::Document;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout};

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
        }
    }
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
    // Step 1: Derive Document from Loro.
    let mut doc = match loki_doc_model::loro_bridge::loro_to_document(loro_doc) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("apply_mutation_and_relayout: loro_to_document failed: {e}");
            return false;
        }
    };

    // Step 2: Restore style catalog from original document (not stored in Loro).
    {
        let Ok(state) = doc_state.lock() else {
            tracing::warn!("apply_mutation_and_relayout: doc_state lock poisoned (catalog)");
            return false;
        };
        if let Some(orig) = &state.document {
            doc.styles = orig.styles.clone();
            doc.source = orig.source.clone();
        }
    }

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
