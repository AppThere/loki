// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! [`RendererState`] — Dioxus context holding the render cache, queue, and
//! scroll signal.
//!
//! Intended to be created inside a `use_hook` call and shared via
//! `use_context` / `provide_context` so that child components and the settle
//! detector can coordinate without prop-drilling.
//!
//! ```ignore
//! // In a parent component:
//! let renderer = use_hook(|| {
//!     RendererState::new(doc.clone(), 800.0, device.clone(), queue.clone())
//! });
//! provide_context(renderer);
//! ```

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_render_cache::{PageCache, PageGeometry, RenderQueue, ScrollState};

use crate::doc_page_source::DocPageSource;

// ── RendererState ─────────────────────────────────────────────────────────────

/// Dioxus context that wires together the page cache, GPU render queue, and
/// scroll signal.
///
/// # Lifecycle
///
/// 1. Create with [`RendererState::new`] inside a `use_hook`.
/// 2. Expose via `provide_context(renderer.clone())`.
/// 3. Call [`on_scroll_event`](crate::scroll_driver::on_scroll_event) from
///    scroll handlers to update `scroll`.
/// 4. Pass [`RendererState::on_settle`] as the callback to
///    [`use_settle_detector`](crate::scroll_driver::use_settle_detector).
///
/// # Cold-tier budget
///
/// The default budget is 256 MiB.  Evictions are logged at `WARN` level so
/// that memory pressure is surfaced without disrupting the user.
const DEFAULT_COLD_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

/// Dioxus context holding the render pipeline state.
///
/// `Clone` is derived so the struct can be moved into multiple closures; the
/// internals use `Arc` for shared ownership.
#[derive(Clone)]
pub struct RendererState {
    /// Shared page texture store.
    pub cache: Arc<Mutex<PageCache>>,
    /// Background render-job dispatcher.
    ///
    /// `Arc` because `RenderQueue` does not implement `Clone` (it owns the
    /// sender half of an mpsc channel) but `RendererState` must be cloneable.
    pub queue: Arc<RenderQueue>,
    /// Scroll position and phase signal, driven by the document scroll handler.
    pub scroll: Signal<ScrollState>,
    /// Document layout and rendering source.
    pub source: Arc<DocPageSource>,
}

impl RendererState {
    /// Creates a new [`RendererState`].
    ///
    /// # Panics (never in library code)
    ///
    /// This function must be called from within a Dioxus component or hook
    /// context because it calls [`use_signal`] internally.
    pub fn new(
        doc: Arc<Document>,
        viewport_height_px: f64,
        device: Arc<wgpu::Device>,
        wgpu_queue: Arc<wgpu::Queue>,
    ) -> Self {
        let source = Arc::new(DocPageSource::new(doc));
        let cache = Arc::new(Mutex::new(PageCache::new(DEFAULT_COLD_BUDGET_BYTES)));
        let queue = Arc::new(RenderQueue::new(
            Arc::clone(&cache),
            Arc::clone(&source) as Arc<dyn loki_render_cache::PageSource>,
            Arc::clone(&device),
            Arc::clone(&wgpu_queue),
        ));
        let scroll = use_signal(|| ScrollState::new(viewport_height_px));
        Self { cache, queue, scroll, source }
    }

    /// Called by the settle detector after each scroll gesture ends.
    ///
    /// Runs [`PageCache::retier`] against the current scroll state, logs the
    /// result, then submits all necessary GPU jobs to the [`RenderQueue`].
    ///
    /// # Tracing
    ///
    /// - `INFO` — settle event with tier-change counts.
    /// - `WARN` — evictions (data loss; Cold-tier textures are dropped).
    #[tracing::instrument(skip(self), fields(page_count = tracing::field::Empty))]
    pub fn on_settle(&self) {
        let layout = self.source.layout();
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
        // Release both guards before queue operations to minimise contention.
        drop(cache);
        drop(scroll_guard);

        tracing::info!(
            rerender  = result.rerender.len(),
            downsample = result.downsample.len(),
            evicted   = result.evicted.len(),
            "retier complete",
        );

        if !result.evicted.is_empty() {
            tracing::warn!(
                evicted = result.evicted.len(),
                "pages evicted from Cold tier (texture data lost); \
                 consider raising the cold-budget or increasing SETTLE_DURATION",
            );
        }

        self.queue.submit(result);
    }
}
