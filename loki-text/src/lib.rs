// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `loki-text` library — Dioxus Native word-processor components and routing.
//!
//! Exposes the module tree for integration testing and potential embedding.
//! The binary entry point lives in `main.rs` and calls [`app::App`].

pub mod app;
pub mod components;
pub mod editing;
pub mod error;
pub mod routes;
pub mod utils;
