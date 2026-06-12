// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Vello rendering backend for the Loki document suite.
//!
//! `loki-vello` is a thin translation layer between the layout pipeline
//! ([`loki_layout::DocumentLayout`]) and the Vello GPU rendering pipeline.
//! It appends draw commands to a [`vello::Scene`] without owning a GPU
//! context, wgpu device, or surface — those belong to the calling application.
//!
//! # Quick start
//!
//! ```no_run
//! use loki_layout::DocumentLayout;
//! use loki_vello::{FontDataCache, paint_layout};
//!
//! fn render(layout: &DocumentLayout) {
//!     let mut scene = vello::Scene::new();
//!     let mut font_cache = FontDataCache::new();
//!     // `None` renders all pages; `Some(0)` renders only the first page.
//!     paint_layout(&mut scene, layout, &mut font_cache, (0.0, 0.0), 1.0, None);
//!     // Pass `scene` to vello::Renderer::render_to_texture(…)
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod band;
pub mod color;
pub mod decor;
pub mod error;
pub mod font_cache;
pub mod glyph;
pub mod image;
pub mod rect;
pub mod scene;

pub use band::{content_max_x, paint_continuous_band};
pub use error::{VelloError, VelloResult};
pub use font_cache::FontDataCache;
pub use scene::{
    CursorPaint, SelectionHandle, SelectionHandleKind, SelectionRect, paint_continuous,
    paint_cursor, paint_layout, paint_paginated, paint_single_page,
};
