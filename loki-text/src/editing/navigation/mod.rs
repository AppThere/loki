// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cursor navigation for the document editor.
//!
//! All functions work in **layout points** (the coordinate space shared by
//! `PaginatedLayout`, `cursor_rect`, and `hit_test_page`).  CSS-pixel
//! conversion happens upstream in the event handler.

mod helpers;
mod public;

pub use public::{
    navigate_down, navigate_end, navigate_home, navigate_left, navigate_right, navigate_up,
};

#[cfg(test)]
mod tests;
