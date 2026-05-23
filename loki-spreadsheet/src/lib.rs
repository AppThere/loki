// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `loki-spreadsheet` library — Dioxus Native spreadsheet components and routing.
//!
//! Exposes the module tree for integration testing and potential embedding.
//! The binary entry point lives in `main.rs` and calls [`app::App`].

// Pre-existing pattern in routes/editor/editor_inner.rs — structural refactor deferred
#![allow(clippy::manual_strip)]

pub mod app;
pub mod error;
pub mod new_document;
pub mod recent_documents;
pub mod routes;
pub mod tabs;
pub mod utils;
