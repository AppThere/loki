// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! [`RendererState`] — Dioxus context holding the render cache and scroll
//! signal.
//!
//! Intended to be created inside a `use_hook` call and shared via
//! `use_context` / `provide_context` so that child components and the settle
//! detector can coordinate without prop-drilling.
//!
//! ```ignore
//! // In a parent component:
//! let renderer = use_hook(|| {
//!     RendererState::new(doc.clone(), 800.0)
//! });
//! provide_context(renderer);
//! ```

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_render_cache::{PageCache, PageGeometry, ScrollState};

use crate::doc_page_source::DocPageSource;

// ── RendererState ─────────────────────────────────────────────────────────────

/// Dioxus context that wires together the page cache and scroll signal.
///
/// GPU texture lifecycle is now managed by `LokiPageSource` instances inside
/// Blitz's `CustomPaintSource` frame loop — `RendererState` only drives the
/// tier-policy metadata via `on_settle`.
///
/// # Lifecycle
///
/// 1. Create with [`RendererState::new`] inside a `use_hook`.
/// 2. Expose via `provide_context(renderer.clone())`.
/// 3. Call [`on_scroll_event`](crate::scroll_driver::on_scroll_event) from
///    scroll handlers to update `scroll`.
/// 4. Pass [`RendererState::on_settle`] as the callback to
///    [`use_settle_detector`](crate::scroll_driver::use_settle_detector).
#[derive(Clone)]
pub struct RendererState {
    /// Shared page tier-and-dirty metadata store.
    pub cache: Arc<Mutex<PageCache>>,
    /// Scroll position and phase signal, driven by the document scroll handler.
    pub scroll: Signal<ScrollState>,
    /// Document layout and page-size source.
    pub source: Arc<DocPageSource>,
}

impl RendererState {
    /// Creates a new [`RendererState`].
    ///
    /// # Panics (never in library code)
    ///
    /// Must be called from within a Dioxus component or hook context because
    /// it calls [`use_signal`] internally.
    pub fn new(
        doc: Arc<Document>,
        viewport_height_px: f64,
    ) -> Self {
        let source = Arc::new(DocPageSource::new(doc));
        let cache = Arc::new(Mutex::new(PageCache::new()));
        let scroll = use_signal(|| ScrollState::new(viewport_height_px));
        Self { cache, scroll, source }
    }

    /// Called by the settle detector after each scroll gesture ends.
    ///
    /// Runs [`PageCache::retier`] against the current scroll state, updating
    /// tier assignments and dirty flags.  `LokiPageSource` instances read
    /// those assignments on their next frame render.
    #[tracing::instrument(skip(self), fields(page_count = tracing::field::Empty))]
    pub fn on_settle(&self) {
        let doc_gen = self.source.current_generation();
        let layout_guard = self.source.layout_for_generation(doc_gen);
        let Some((_, layout)) = layout_guard.as_ref() else {
            tracing::warn!("RendererState::on_settle: layout unavailable");
            return;
        };
        let mut current_top = 0.0;
        let mut pages = Vec::with_capacity(layout.pages.len());
        for (i, p) in layout.pages.iter().enumerate() {
            let h = p.page_size.height as f64;
            pages.push(PageGeometry {
                index: i as u32,
                top_px: current_top,
                bottom_px: current_top + h,
            });
            current_top += h + loki_theme::tokens::PAGE_GAP_PX as f64;
        }
        drop(layout_guard);
        tracing::Span::current().record("page_count", pages.len());

        let scroll_guard = self.scroll.read();
        let mut cache = match self.cache.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!("RendererState::on_settle: cache lock poisoned; using inner");
                e.into_inner()
            }
        };

        let result = cache.retier(&pages, &scroll_guard);
        drop(cache);
        drop(scroll_guard);

        let (hot, warm, cold) = self
            .cache
            .lock()
            .map(|g| g.page_count_by_tier())
            .unwrap_or((0, 0, 0));

        tracing::info!(
            rerender   = result.rerender.len(),
            downsample = result.downsample.len(),
            hot,
            warm,
            cold,
            "retier complete",
        );
    }
}
