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
//! # Canvas sizing
//!
//! The canvas element CSS size is derived from the document's `<w:pgSz>` page
//! dimensions (via loki_layout::PaginatedLayout;
//! the CSS standard conversion: `1 pt = 96/72 CSS px`.  This ensures that
//! documents of any page size (A4, Letter, 6×9 book, etc.) get a canvas that
//! exactly matches their page boundary.
//!
//! # Click coordinate strategy
//!
//! `PageCanvas.onmousedown` uses `evt.element_coordinates()` to obtain the
//! click position relative to the canvas element's top-left corner.  This is
//! implemented by a local patch to `dioxus-native-dom` (`patches/dioxus-native-dom`)
//! that calls `Node::absolute_position(0, 0)` on the event target at dispatch
//! time.  `absolute_position` walks up the layout tree subtracting each
//! ancestor's `scroll_offset`, so the result is correct even after the user
//! has scrolled the document container.  No hardcoded window width or scroll
//! offset signal is needed.
//!
//! `visible_rect` is `None` because `node.scroll_offset` is internal to
//! blitz-dom and no public API in dioxus-native-0.7.4 exposes it to Dioxus
//! components.  Blitz handles `WindowEvent::MouseWheel` in blitz-shell directly
//! (`scroll_node_by_has_changed`, blitz-shell-0.2.3/src/window.rs:388) without
//! routing through the Dioxus event system, so `onwheel` handlers never fire.
//! The scroll container in `editor.rs` uses `height: calc(100vh - Npx)` to give
//! taffy a concrete `Dimension::Length` — without this, flex-chain propagation
//! leaves the height indefinite and `scroll_height()` stays zero.
//!
//! TODO(partial-render): wire scroll_offset → visible_rect → LokiDocumentSource
//! clip region once Blitz exposes a scroll-position hook to Dioxus components.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use kurbo::Rect;
use loki_doc_model::document::Document;
use loki_layout::{layout_document, DocumentLayout, FontResources, LayoutMode, LayoutOptions};
use loki_theme::tokens;

use crate::components::document_source::{DocumentState, LokiDocumentSource};
use crate::editing::cursor::{CursorState, DocumentPosition};

// ── WgpuSurfaceProps ──────────────────────────────────────────────────────────

/// Props for [`WgpuSurface`].
///
/// [`Document`] does not implement [`PartialEq`], so the props struct provides
/// a conservative `PartialEq` (always `false`) ensuring re-renders are never
/// incorrectly skipped.
#[derive(Clone, Props)]
pub struct WgpuSurfaceProps {
    /// Shared document state created by the parent `Editor` component.
    ///
    /// Passed down so the parent can close over it in mouse/keyboard handlers
    /// (to read `paginated_layout` for hit-testing and to bump the generation
    /// counter after mutations).
    pub doc_state: Arc<Mutex<DocumentState>>,

    /// Document to render.  `None` shows a placeholder until loading completes.
    pub document: Option<Document>,

    /// Options used for document layout generation.
    pub layout_opts: LayoutOptions,

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

    /// Current cursor and selection state from the editing layer.
    ///
    /// `None` in read-only mode. When `Some`, the GPU canvas paints a cursor
    /// caret and optional selection highlights on top of the document content.
    pub cursor_state: Option<CursorState>,

    /// Callback fired when a page is clicked.
    pub on_mousedown: EventHandler<DocumentPosition>,
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
    /// Canvas width in CSS logical pixels, derived from the document page size.
    page_width_px: f32,
    /// Canvas height in CSS logical pixels, derived from the document page size.
    page_height_px: f32,
    /// Current cursor state, used to trigger re-renders.
    pub cursor_state: Option<CursorState>,
    /// Callback for clicks on this specific page.
    pub on_mousedown: EventHandler<DocumentPosition>,
}

impl PartialEq for PageCanvasProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.page_index == other.page_index
            && self.page_width_px == other.page_width_px
            && self.page_height_px == other.page_height_px
            && self.cursor_state == other.cursor_state
    }
}

/// A single-page GPU canvas.  Calls `use_wgpu` exactly once per instance.
///
/// Canvas dimensions are taken from `page_width_px` / `page_height_px` props,
/// which are derived from the document's `<w:pgSz>` page size converted from
/// points to CSS px (`pt × 96/72`).
#[allow(non_snake_case)]
fn PageCanvas(props: PageCanvasProps) -> Element {
    let source_state = props.doc_state.clone();
    let page_index = props.page_index;
    let page_height_px = props.page_height_px;
    let canvas_id = use_wgpu(move || LokiDocumentSource::new(source_state, page_index));

    rsx! {
        canvas {
            "src": "{canvas_id}",
            // Dummy attribute that changes with the cursor to ensure Blitz
            // marks the node as dirty and re-calls render().
            "data-cursor": if let Some(cs) = &props.cursor_state {
                format!("{:?}-{:?}", cs.anchor, cs.focus)
            } else {
                "none".to_string()
            },
            onmousedown: move |evt| {
                // element_coordinates() gives the click position relative to the
                // canvas element's top-left corner, computed via
                // Node::absolute_position in the patched dioxus-native-dom.
                // This correctly accounts for scroll offset and centering without
                // needing hardcoded window width or scroll offset props.
                let elem = evt.element_coordinates();
                let x_pt = elem.x as f32 * (72.0 / 96.0);
                let y_pt = elem.y as f32 * (72.0 / 96.0);

                let Ok(state) = props.doc_state.lock() else { return };
                if let Some(ref layout) = state.paginated_layout {
                    if let Some(pos) = crate::editing::hit_test::hit_test_page(
                        props.page_index,
                        x_pt,
                        y_pt,
                        layout,
                    ) {
                        props.on_mousedown.call(pos);
                    }
                }
            },
            style: format!(
                "width: {w}px; height: {h}px; display: block;",
                w = props.page_width_px,
                h = page_height_px,
            ),
        }
    }
}

// ── WgpuSurface ───────────────────────────────────────────────────────────────

/// Top-level document canvas component.
///
/// Owns the shared [`DocumentState`], computes the page layout to determine
/// page count and page dimensions, and renders one [`PageCanvas`] per page
/// stacked vertically.  When `document` is `None` or the layout yields zero
/// pages, an "Opening document…" placeholder is shown instead.
#[allow(non_snake_case)]
pub fn WgpuSurface(props: WgpuSurfaceProps) -> Element {
    let WgpuSurfaceProps { doc_state, document, layout_opts, visible_rect, cursor_state, .. } = props;

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

    // Page count and CSS pixel page dimensions computed synchronously when the
    // document key changes so the RSX below sees the updated values in the same
    // render frame.
    let page_count_rc: Rc<RefCell<usize>> = use_hook(|| Rc::new(RefCell::new(0usize)));
    // Falls back to A4 (794 × 1123 CSS px) until a document is loaded.
    let page_dims_rc: Rc<RefCell<(f32, f32)>> =
        use_hook(|| Rc::new(RefCell::new((tokens::PAGE_WIDTH_PX, tokens::PAGE_HEIGHT_PX))));

    if key_changed {
        let (new_count, new_dims, new_layout) = if let Some(doc) = document.as_ref() {
            let layout = layout_document(
                &mut *font_resources.borrow_mut(),
                doc,
                LayoutMode::Paginated,
                1.0,
                &layout_opts,
            );
            match layout {
                DocumentLayout::Paginated(pl) => {
                    // Convert loki-layout points (1/72 in) to CSS pixels (1/96 in).
                    let w_px = pl.page_size.width * (96.0 / 72.0);
                    let h_px = pl.page_size.height * (96.0 / 72.0);
                    (pl.pages.len(), (w_px, h_px), Some(Arc::new(pl)))
                }
                _ => (0, (tokens::PAGE_WIDTH_PX, tokens::PAGE_HEIGHT_PX), None),
            }
        } else {
            (0, (tokens::PAGE_WIDTH_PX, tokens::PAGE_HEIGHT_PX), None)
        };
        *page_count_rc.borrow_mut() = new_count;
        *page_dims_rc.borrow_mut() = new_dims;

        if let Ok(mut state) = doc_state.lock() {
            state.document = document;
            state.generation = state.generation.wrapping_add(1);
            state.page_count = new_count;
            let (pw, ph) = new_dims;
            state.page_width_px = pw;
            state.page_height_px = ph;
            state.paginated_layout = new_layout;
        }
    }

    // Propagate visible_rect + cursor_state + preserve_for_editing into shared
    // state (these can change without key_changed being true).
    if let Ok(mut state) = doc_state.lock() {
        state.visible_rect = visible_rect;
        state.cursor_state = cursor_state.clone();
        state.preserve_for_editing = layout_opts.preserve_for_editing;
    }

    let current_page_count = *page_count_rc.borrow();
    let (current_page_width_px, current_page_height_px) = *page_dims_rc.borrow();

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
                    page_width_px: current_page_width_px,
                    page_height_px: current_page_height_px,
                    cursor_state: cursor_state.clone(),
                    on_mousedown: move |pos| props.on_mousedown.call(pos),
                }
            }
        }
    }
}
