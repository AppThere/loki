// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Canvas-coordinate to document-position hit testing.
//!
//! # Coordinate-transform strategy
//!
//! **Strategy C — calculated from known layout values.**
//!
//! Neither `use_mounted` / `MountedData::get_client_rect()` nor
//! `offset_x` / `offset_y` are available in dioxus-native-dom 0.7.4
//! (both are `unimplemented!()`). Pointer events provide only
//! `ClientPoint` (window-relative logical pixels). The canvas origin
//! is therefore computed from:
//!
//! - `canvas_origin.x` = `(window_inner_width_px − page_width_px) / 2.0`
//!   (pages are flex-centered; `window_inner_width_px` defaults to 1280 px
//!   and must be updated when a window-size API becomes available in Blitz).
//! - `canvas_origin.y` = `TOOLBAR_HEIGHT_TOP + SPACE_6` (exact from tokens).
//!
//! - `scroll_offset` = 0.0 (Blitz does not expose `node.scroll_offset` to
//!   Dioxus components; wired as a TODO once the API is available).
//!
//! All geometry inside this function works in layout **points** (1 pt = 1/72 in).
//! The conversion from CSS logical pixels is applied once at entry:
//! `pt = px × (72/96)`.

mod document;
mod page;

pub use document::hit_test_document;
pub use page::hit_test_page;

/// CSS pixels → layout points scale factor (72 dpi / 96 dpi).
pub(super) const PX_TO_PT: f32 = 72.0 / 96.0;
