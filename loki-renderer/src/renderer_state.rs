// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`RendererState`] — Dioxus context holding the page source and shared Vello
//! renderer.

use std::sync::{Arc, Mutex};

use loki_doc_model::document::Document;

use crate::doc_page_source::DocPageSource;

// ── RendererState ─────────────────────────────────────────────────────────────

/// Dioxus context that wires together the page source and shared Vello renderer.
#[derive(Clone)]
pub struct RendererState {
    /// Document layout and page-size source.
    pub source: Arc<DocPageSource>,
    /// Shared Vello renderer — created lazily by the first `LokiPageSource`
    /// to call `resume()`.  All page sources for the same document share this.
    pub shared_renderer: Arc<Mutex<Option<vello::Renderer>>>,
}

impl RendererState {
    /// Creates a new [`RendererState`] for `doc`.
    pub fn new(doc: Arc<Document>) -> Self {
        Self {
            source: Arc::new(DocPageSource::new(doc)),
            shared_renderer: Arc::new(Mutex::new(None)),
        }
    }
}
