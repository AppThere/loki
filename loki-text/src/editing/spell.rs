// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Active spell-check state for the editor (re-exported from `loki-renderer`).
//!
//! The active dictionary is held in `loki_renderer::spell` so that both the
//! renderer's paint layout and the editor's hit-test layout read one source of
//! truth. This module re-exports it under the editing namespace where the
//! editing layout paths and the app root reference it.

pub use loki_renderer::spell::{active, set_active};
