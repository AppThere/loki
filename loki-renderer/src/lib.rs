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
//! | [`gpu_probe`] | which GPU adapter the paint path resumed on (Spec 08 T2.0) |
//! | [`dpr_probe`] | what display scale factor the paint path renders at (Spec 08 R27) |

#![forbid(unsafe_code)]

pub mod doc_page_source;
mod doc_page_source_reflow;
mod doc_page_source_scale;
pub mod document_view;
// Deliberately ungated. `record` is only ever called from the GPU paint path,
// but `observed_adapter` must resolve on every target so the application-side
// sensor is one code path rather than two — the shape the android-check job
// caught in `document_view.rs`, where a module was ungated and its import was
// not. On the Android CPU path it simply always answers `None`, which is true.
pub mod dpr_probe;
pub mod gpu_probe;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub mod page_paint_source;
pub(crate) mod page_source_impl;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub(crate) mod page_tile;
mod view_types;
// The HTML-flow fallback view is only compiled on the Android CPU path; GPU
// targets render reflow mode through the layout engine (RenderMode::Reflow).
#[cfg(all(target_os = "android", not(android_gpu)))]
pub(crate) mod reflow_view;
pub mod render_layout;
pub mod renderer_state;
pub mod revision;
// Gated with `tile_plan`, which `zoom_capability::apply_to` reads page sizes
// from, and whose only caller is `document_view`'s GPU branch. Ungated, it broke
// the Android CPU build — the same "a module was ungated and its import was
// not" shape this file warns about above, repeated in the module added for T5.4.
#[cfg(any(not(target_os = "android"), android_gpu))]
mod scale_resolve;
pub mod spell;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub(crate) mod tile_key;
#[cfg(any(not(target_os = "android"), android_gpu))]
pub(crate) mod tile_plan;
pub(crate) mod vello_init;
pub mod zoom_capability;

pub use doc_page_source::DocPageSource;
pub use document_view::{
    DocumentView, DocumentViewProps, RendererCursorPos, TileContext, ViewMode,
};
pub use render_layout::{RenderLayout, RenderMode};
pub use renderer_state::RendererState;
