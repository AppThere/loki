// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`RendererState`] — Dioxus context holding the page source.

use std::sync::Arc;

use loki_doc_model::document::Document;

use crate::doc_page_source::DocPageSource;

// ── RendererState ─────────────────────────────────────────────────────────────

/// Dioxus context that carries the page source to every tile.
///
/// # There is no renderer here any more
///
/// This used to also carry a `shared_renderer: Arc<Mutex<Option<vello::Renderer>>>`,
/// lazily built by the first `LokiPageSource` to resume and shared by every tile
/// of the document. Tiles now render on **Blitz's** renderer, reached through
/// `CustomPaintCtx::renderer_mut()` during the paint callback, so the second
/// renderer — and with it ~165 MiB of fixed Vello scratch buffers, allocated
/// whatever the page contained — is gone. See `docs/patches.md`
/// ("`CustomPaintCtx::renderer_mut`").
#[derive(Clone)]
pub struct RendererState {
    /// Document layout and page-size source.
    pub source: Arc<DocPageSource>,
}

impl RendererState {
    /// Creates a new [`RendererState`] for `doc`.
    pub fn new(doc: Arc<Document>) -> Self {
        Self {
            source: Arc::new(DocPageSource::new(doc)),
        }
    }
}
