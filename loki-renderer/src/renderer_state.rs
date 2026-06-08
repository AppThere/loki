// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`RendererState`] — Dioxus context holding the render cache, scroll signal,
//! phase sender, and shared Vello renderer.

use std::sync::{Arc, Mutex};

use appthere_canvas::{PageCache, PageGeometry, PageIndex, ScrollPhase, ScrollState};
use dioxus::prelude::{ReadableExt, Signal};
use loki_doc_model::document::Document;
use tokio::sync::watch;

use crate::doc_page_source::DocPageSource;

// ── RendererState ─────────────────────────────────────────────────────────────

/// Dioxus context that wires together the page cache, scroll signal, and
/// shared Vello renderer.
#[derive(Clone)]
pub struct RendererState {
    /// Shared page tier-and-dirty metadata store.
    pub cache: Arc<Mutex<PageCache<PageIndex>>>,
    /// Scroll position and phase signal.
    pub scroll: Signal<ScrollState>,
    /// Document layout and page-size source.
    pub source: Arc<DocPageSource>,
    /// Watch sender for scroll phase — passed to `on_scroll_event` to drive
    /// the event-based settle detector.
    pub phase_tx: Arc<watch::Sender<ScrollPhase>>,
    /// Shared Vello renderer — created lazily by the first `LokiPageSource`
    /// to call `resume()`.  All page sources for the same document share this.
    pub shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
}

impl RendererState {
    /// Creates a new [`RendererState`] from values already initialised at the
    /// component top level.
    ///
    /// `scroll` must be a `Signal` created with `use_signal` in the calling
    /// component before this function is called.  Hooks must not be called
    /// inside `use_hook` closures; this function therefore accepts `scroll`
    /// as a parameter instead of creating it internally.
    pub fn new(doc: Arc<Document>, scroll: Signal<ScrollState>) -> Self {
        let source = Arc::new(DocPageSource::new(doc));
        let cache = Arc::new(Mutex::new(PageCache::new()));
        let (tx, _rx) = watch::channel(ScrollPhase::Idle);
        let phase_tx = Arc::new(tx);
        let shared_renderer = Arc::new(Mutex::new(None));
        Self {
            cache,
            scroll,
            source,
            phase_tx,
            shared_renderer,
        }
    }

    /// Called by the settle detector after each scroll gesture ends.
    #[tracing::instrument(skip(self), fields(page_count = tracing::field::Empty))]
    pub fn on_settle(&self) {
        let doc_gen = self.source.current_generation();
        let layout_guard = self.source.layout_for_generation(doc_gen);
        let Some((_, layout)) = layout_guard.as_ref() else {
            tracing::warn!("RendererState::on_settle: layout unavailable");
            return;
        };
        // loki-layout page dimensions are in typographic points (1pt = 1/72 inch).
        // CSS pixels assume 96dpi. Conversion: 1pt = 96/72 CSS px.
        // ScrollState.viewport_top_px is in CSS px; these must match.
        const PTS_TO_CSS_PX: f64 = 96.0 / 72.0;
        let mut current_top = 0.0;
        let mut pages = Vec::with_capacity(layout.pages.len());
        for (i, p) in layout.pages.iter().enumerate() {
            let h_px = p.page_size.height as f64 * PTS_TO_CSS_PX;
            pages.push(PageGeometry {
                index: PageIndex(i as u32),
                top_px: current_top,
                bottom_px: current_top + h_px,
            });
            current_top += h_px + appthere_ui::tokens::PAGE_GAP_PX as f64;
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
            rerender = result.rerender.len(),
            downsample = result.downsample.len(),
            hot,
            warm,
            cold,
            "retier complete",
        );
    }
}
