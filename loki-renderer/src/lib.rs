// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Dioxus integration and orchestration layer for the Loki page renderer.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`doc_page_source`] | Layout + page-size source backed by `loki-doc-model` |
//! | [`page_paint_source`] | Per-page `CustomPaintSource` (`LokiPageSource`) |
//! | [`renderer_state`] | [`RendererState`] — Dioxus context holding the page source + renderer |
//! | [`document_view`] | [`DocumentView`] root component |

#![forbid(unsafe_code)]

pub mod doc_page_source;
pub mod document_view;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub mod page_paint_source;
pub(crate) mod page_source_impl;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub(crate) mod page_tile;
// The HTML-flow fallback view is only compiled on the Android CPU path; GPU
// targets render reflow mode through the layout engine (RenderMode::Reflow).
#[cfg(all(target_os = "android", not(android_gpu)))]
pub(crate) mod reflow_view;
pub mod render_layout;
pub mod renderer_state;
pub mod spell;
pub(crate) mod vello_init;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub(crate) mod virtualize;

pub use doc_page_source::DocPageSource;
pub use document_view::{
    DocumentView, DocumentViewProps, RendererCursorPos, TileContext, ViewMode,
};
pub use render_layout::{RenderLayout, RenderMode};
pub use renderer_state::RendererState;
