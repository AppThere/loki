// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Editing-layer types and logic for the Loki document editor.
//!
//! This module contains cursor state representation, coordinate-space hit
//! testing, and the helper that derives a stable Loro [`Cursor`] from a
//! layout-based [`DocumentPosition`].
//!
//! [`Cursor`]: loro::Cursor

pub mod cursor;
pub mod hit_test;
pub mod navigation;
mod navigation_find;
pub mod page_locate;
pub mod reflow_nav;
pub mod relayout;
pub mod saved_state;
pub mod selected_object;
pub mod selection_handles;
pub mod spell;
pub mod state;
pub mod touch;
pub mod viewport;
pub mod word_count;
