// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Dioxus signal integration and orchestration layer for the Loki render cache.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`scroll_driver`] | Re-exports from `appthere_canvas::dioxus::scroll_driver` |
//! | [`doc_page_source`] | Layout + page-size source backed by `loki-doc-model` |
//! | [`page_paint_source`] | Per-page `CustomPaintSource` (`LokiPageSource`) |
//! | [`renderer_state`] | [`RendererState`] — Dioxus context holding cache + scroll + renderer |
//! | [`document_view`] | [`DocumentView`] root component |

pub mod doc_page_source;
pub mod document_view;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub mod page_paint_source;
#[cfg(all(target_os = "android", not(android_gpu)))]
pub(crate) mod page_tile_cpu;
pub mod renderer_state;
pub mod scroll_driver;

pub use doc_page_source::DocPageSource;
pub use document_view::{DocumentView, DocumentViewProps, RendererCursorPos};
pub use renderer_state::RendererState;
pub use scroll_driver::{on_scroll_event, use_settle_detector};
