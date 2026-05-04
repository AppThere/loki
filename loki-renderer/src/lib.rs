// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Dioxus signal integration and orchestration layer for the Loki render cache.
//!
//! # Crate overview
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`scroll_driver`] | Dioxus signal helpers: [`on_scroll_event`], [`use_settle_detector`] |
//! | [`doc_page_source`] | Layout + page-size source backed by `loki-doc-model` |
//! | [`page_paint_source`] | Per-page `CustomPaintSource` (`LokiPageSource`) |
//! | [`renderer_state`] | [`RendererState`] — Dioxus context holding cache + scroll signal |
//! | [`document_view`] | [`DocumentView`] root component |

pub mod doc_page_source;
pub mod document_view;
pub mod page_paint_source;
pub mod renderer_state;
pub mod scroll_driver;

pub use document_view::{DocumentView, DocumentViewProps};
pub use doc_page_source::DocPageSource;
pub use renderer_state::RendererState;
pub use scroll_driver::{on_scroll_event, use_settle_detector};
