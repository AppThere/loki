// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Re-exports of the event-driven scroll helpers from `appthere-canvas`.
//!
//! The implementation lives in [`appthere_canvas::dioxus::scroll_driver`].
//! This module exists for API stability — existing code importing from
//! `loki_renderer::scroll_driver` continues to compile unchanged.

pub use appthere_canvas::dioxus::scroll_driver::{on_scroll_event, use_settle_detector};
