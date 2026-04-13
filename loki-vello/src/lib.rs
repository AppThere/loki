// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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
//!     paint_layout(&mut scene, layout, &mut font_cache, (0.0, 0.0), 1.0);
//!     // Pass `scene` to vello::Renderer::render_to_texture(…)
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod decor;
pub mod error;
pub mod font_cache;
pub mod glyph;
pub mod image;
pub mod rect;
pub mod scene;

pub use error::{VelloError, VelloResult};
pub use font_cache::FontDataCache;
pub use scene::{paint_continuous, paint_layout, paint_paginated};
