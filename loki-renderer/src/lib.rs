// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Dioxus signal integration and tracing orchestration layer for the Loki render cache.
//!
//! # Crate overview
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`scroll_driver`] | Dioxus signal helpers: [`on_scroll_event`], [`use_settle_detector`] |
//! | [`doc_page_source`] | [`PageSource`](loki_render_cache::PageSource) bridge over `loki-doc-model` |
//! | [`renderer_state`] | [`RendererState`] — Dioxus context holding cache + queue + scroll signal |

pub mod doc_page_source;
pub mod document_view;
pub mod renderer_state;
pub mod scroll_driver;

pub use document_view::{DocumentView, DocumentViewProps};

pub use doc_page_source::DocPageSource;
pub use renderer_state::RendererState;
pub use scroll_driver::{on_scroll_event, use_settle_detector};
