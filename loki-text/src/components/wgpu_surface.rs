// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! WGPU document canvas component.
//!
//! [`WgpuSurface`] is the top-level document canvas: it computes the page count
//! for the current document and creates one [`PageCanvas`] per page, each of
//! which owns a separate [`LokiDocumentSource`] and `<canvas>` element.  Pages
//! are stacked vertically with [`loki_theme::tokens::PAGE_GAP_PX`] spacing inside
//! a parent scroll container provided by the editor shell.
//!
//! When no document is loaded (`document: None`) or the layout yields zero pages,
//! a placeholder `div` is shown so that no wgpu contexts are created unnecessarily.
//!
//! # Hook constraint
//!
//! `use_wgpu` (like all Dioxus hooks) must be called a fixed number of times per
//! component instance.  A loop-based `use_wgpu` call in `WgpuSurface` would
//! violate this invariant.  The solution is a dedicated `PageCanvas` component:
//! each instance calls `use_wgpu` exactly once, and Dioxus's key-based
//! reconciliation mounts/unmounts `PageCanvas` instances as `page_count` changes.
//!
//! # Integration seam
//!
//! `visible_rect` is preserved as a `None` placeholder.  Blitz's scroll events
//! are handled directly in blitz-shell (`MouseWheel` → `scroll_node_by_has_changed`)
//! without going through the Dioxus event system, so `onwheel` handlers never
//! fire and the scroll offset is not observable from a Dioxus component.
//! Native `overflow-y: auto` on the scroll container does work (blitz-paint
//! applies `node.scroll_offset` as a translation), but the offset cannot be
//! read back to populate `visible_rect`.
//!
//! TODO(partial-render): wire scroll-position → visible_rect → LokiDocumentSource
//! clip region once blitz exposes scroll offset to Dioxus components.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use kurbo::Rect;
use loki_doc_model::document::Document;
use loki_layout::{layout_document, DocumentLayout, FontResources, LayoutMode};
use loki_theme::tokens;

use crate::components::document_source::{DocumentState, LokiDocumentSource};

// ── WgpuSurfaceProps ──────────────────────────────────────────────────────────

/// Props for [`WgpuSurface`].
///
/// [`Document`] does not implement [`PartialEq`], so the props struct provides
/// a conservative `PartialEq` (always `false`) ensuring re-renders are never
/// incorrectly skipped.
#[derive(Clone, Props)]
pub struct WgpuSurfaceProps {
    /// Document to render.  `None` shows a placeholder until loading completes.
    pub document: Option<Document>,

    /// Currently visible portion of the document canvas in document-space
    /// coordinates.
    ///
    /// # Future work
    ///
    /// TODO(partial-render): Populate with the current scroll viewport.
    /// [`LokiDocumentSource`] will use this to clip items before scene building,
    /// reducing GPU work for large documents.  Leave as `None` until scroll
    /// infrastructure is implemented.
    pub visible_rect: Option<Rect>,
}

// Document does not implement PartialEq; conservatively always re-render.
impl PartialEq for WgpuSurfaceProps {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

// ── PageCanvas ────────────────────────────────────────────────────────────────

#[derive(Clone, Props)]
struct PageCanvasProps {
    doc_state: Arc<Mutex<DocumentState>>,
    page_index: usize,
}

impl PartialEq for PageCanvasProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state) && self.page_index == other.page_index
    }
}

/// A single-page GPU canvas.  Calls `use_wgpu` exactly once per instance.
///
/// Canvas width is responsive: `min(PAGE_WIDTH_PX, 100vw − 2×SPACE_4)`.
/// Aspect ratio is fixed at the A4 page ratio so the canvas height scales
/// proportionally on narrow viewports.
#[allow(non_snake_case)]
fn PageCanvas(props: PageCanvasProps) -> Element {
    let source_state = props.doc_state.clone();
    let page_index = props.page_index;
    let canvas_id = use_wgpu(move || LokiDocumentSource::new(source_state, page_index));

    rsx! {
        canvas {
            "src": "{canvas_id}",
            style: format!(
                "width: {w}px; height: {h}px; display: block;",
                w = tokens::PAGE_WIDTH_PX,
                h = tokens::PAGE_HEIGHT_PX,
            ),
        }
    }
}

// ── WgpuSurface ───────────────────────────────────────────────────────────────

/// Top-level document canvas component.
///
/// Owns the shared [`DocumentState`], computes the page layout to determine
/// page count, and renders one [`PageCanvas`] per page stacked vertically.
/// When `document` is `None` or the layout yields zero pages, an
/// "Opening document…" placeholder is shown instead.
#[allow(non_snake_case)]
pub fn WgpuSurface(props: WgpuSurfaceProps) -> Element {
    let WgpuSurfaceProps { document, visible_rect } = props;

    // Shared state between this component and all LokiDocumentSource instances.
    let doc_state: Arc<Mutex<DocumentState>> = use_hook(|| {
        Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            page_count: 0,
            canvas_width: 0.0,
            visible_rect: None,
        }))
    });

    // Cheap comparable key for the current document.
    // Using (title, section_count) avoids deriving PartialEq on Document.
    let new_key: (Option<String>, usize) = (
        document.as_ref().and_then(|d| d.meta.title.clone()),
        document.as_ref().map(|d| d.sections.len()).unwrap_or(0),
    );

    let prev_key: Rc<RefCell<(Option<String>, usize)>> =
        use_hook(|| Rc::new(RefCell::new((None, 0))));

    let key_changed = *prev_key.borrow() != new_key;
    if key_changed {
        *prev_key.borrow_mut() = new_key.clone();
    }

    // FontResources cached for this component's own layout call (page count).
    // This is separate from the FontResources inside each LokiDocumentSource;
    // the duplication is intentional — WgpuSurface needs page count before
    // GPU canvases are created (hook count constraint prevents dynamic use_wgpu).
    let font_resources: Rc<RefCell<FontResources>> =
        use_hook(|| Rc::new(RefCell::new(FontResources::new())));

    // Page count computed synchronously when document key changes so the RSX
    // below sees the updated value in the same render frame.
    let page_count_rc: Rc<RefCell<usize>> = use_hook(|| Rc::new(RefCell::new(0usize)));

    if key_changed {
        let new_count = if let Some(doc) = document.as_ref() {
            let layout = layout_document(
                &mut *font_resources.borrow_mut(),
                doc,
                LayoutMode::Paginated,
                1.0,
            );
            match &layout {
                DocumentLayout::Paginated(pl) => pl.pages.len(),
                _ => 0,
            }
        } else {
            0
        };
        *page_count_rc.borrow_mut() = new_count;
    }

    // Propagate document + visible_rect into shared state.
    if let Ok(mut state) = doc_state.lock() {
        if key_changed {
            state.document = document;
            state.generation = state.generation.wrapping_add(1);
            state.page_count = *page_count_rc.borrow();
        }
        state.visible_rect = visible_rect;
    }

    let current_page_count = *page_count_rc.borrow();

    if current_page_count == 0 {
        return rsx! {
            div {
                style: format!(
                    "display: flex; justify-content: center; align-items: center; \
                     padding: {p}px; color: {fg};",
                    p  = tokens::SPACE_8,
                    fg = tokens::COLOR_TEXT_SECONDARY,
                ),
                "Opening document\u{2026}"
            }
        };
    }

    rsx! {
        for page_idx in 0..current_page_count {
            div {
                key: "{page_idx}",
                style: format!(
                    "display: flex; justify-content: center; padding-bottom: {gap}px;",
                    gap = tokens::PAGE_GAP_PX,
                ),
                PageCanvas {
                    doc_state: Arc::clone(&doc_state),
                    page_index: page_idx,
                }
            }
        }
    }
}
