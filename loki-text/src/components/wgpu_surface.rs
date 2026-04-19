// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! WGPU document canvas component.
//!
//! [`WgpuSurface`] bridges the Dioxus component tree and the Loki GPU rendering
//! pipeline.  It registers a [`LokiDocumentSource`] with Blitz's renderer via
//! `dioxus::native::use_wgpu`, then emits a `<canvas src="{id}">` element that
//! Blitz intercepts to invoke `CustomPaintSource::render` each frame.
//!
//! Document state is shared with [`LokiDocumentSource`] through an
//! `Arc<Mutex<DocumentState>>`.  A cheap key comparison (title + section count)
//! detects document changes and bumps a generation counter, avoiding redundant
//! `layout_document` calls on frames where nothing has changed.
//!
//! # Integration seam
//!
//! `visible_rect` is preserved as a `None` placeholder until scroll
//! infrastructure is implemented.  When available, pass the current scroll
//! viewport into [`DocumentState::visible_rect`] so that
//! [`LokiDocumentSource::render`] can cull items outside the viewport before
//! building the Vello scene.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::native::use_wgpu;
use dioxus::prelude::*;
use kurbo::Rect;
use loki_doc_model::document::Document;

use crate::components::document_source::{DocumentState, LokiDocumentSource};

// ── WgpuSurfaceProps ──────────────────────────────────────────────────────────

/// Props for [`WgpuSurface`].
///
/// [`Document`] does not implement [`PartialEq`], so the props struct provides
/// a conservative `PartialEq` (always `false`) ensuring re-renders are never
/// incorrectly skipped.
#[derive(Clone, Props)]
pub struct WgpuSurfaceProps {
    /// Document to render.  `None` shows a blank A4 placeholder.
    pub document: Option<Document>,

    /// Currently visible portion of the document canvas in document-space
    /// coordinates.
    ///
    /// # Future work
    ///
    /// Populate with the current scroll viewport.  [`LokiDocumentSource`] will
    /// use this to clip items before scene building, reducing GPU work for large
    /// documents.  Leave as `None` until scroll infrastructure is implemented.
    pub visible_rect: Option<Rect>,
}

// Document does not implement PartialEq; conservatively always re-render.
impl PartialEq for WgpuSurfaceProps {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

// ── WgpuSurface ───────────────────────────────────────────────────────────────

/// WGPU document canvas component.
///
/// Registers a [`LokiDocumentSource`] via `use_wgpu` and renders a
/// `<canvas src="{id}">` element.  Blitz intercepts the `src` attribute to call
/// `CustomPaintSource::render` on the registered source each frame.
///
/// Tracks document changes via a cheap key `(title, section_count)` and bumps a
/// generation counter on changes so [`LokiDocumentSource`] can invalidate its
/// layout cache.
#[allow(non_snake_case)]
pub fn WgpuSurface(props: WgpuSurfaceProps) -> Element {
    let WgpuSurfaceProps { document, visible_rect } = props;

    // Shared state between this component and LokiDocumentSource.
    // Arc is Clone so use_hook returns the same underlying pointer each render.
    let doc_state: Arc<Mutex<DocumentState>> = use_hook(|| {
        Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            visible_rect: None,
        }))
    });

    // Cheap comparable key for the current document.
    // Using (title, section_count) avoids deriving PartialEq on Document.
    let new_key: (Option<String>, usize) = (
        document.as_ref().and_then(|d| d.meta.title.clone()),
        document.as_ref().map(|d| d.sections.len()).unwrap_or(0),
    );

    // Track the previously seen key across renders.
    let prev_key: Rc<RefCell<(Option<String>, usize)>> =
        use_hook(|| Rc::new(RefCell::new((None, 0))));

    let key_changed = *prev_key.borrow() != new_key;
    if key_changed {
        *prev_key.borrow_mut() = new_key.clone();
    }

    // Update shared state — document + generation bump on change, visible_rect
    // always updated (cheap copy).
    if let Ok(mut state) = doc_state.lock() {
        if key_changed {
            state.document = document;
            state.generation = state.generation.wrapping_add(1);
        }
        state.visible_rect = visible_rect;
    }

    // Register LokiDocumentSource with Blitz's renderer once per component
    // lifetime.  The closure runs only on the first render (use_hook_with_cleanup
    // semantics); subsequent renders reuse the returned id.  The Arc is moved
    // into the closure so the closure is 'static.
    let source_state = Arc::clone(&doc_state);
    let canvas_id = use_wgpu(move || LokiDocumentSource::new(source_state));

    rsx! {
        // Blitz intercepts <canvas src="{id}"> and calls
        // CustomPaintSource::render on the registered source each frame.
        // "src" is quoted because it is not a typed Dioxus canvas attribute;
        // blitz-dom parses it as a u64 and routes it to the paint source.
        canvas {
            "src": "{canvas_id}",
            style: "width: 100%; height: 100%; display: block;",
        }
    }
}
