// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `loki-text` — Dioxus Native word-processor application.
//!
//! This binary is the entry point for the Loki word-processor on desktop
//! (macOS, Windows, Linux) and mobile (Android, iOS) platforms.  A single
//! Dioxus Native codebase targets all runtimes via responsive design.
//!
//! # Module layout
//!
//! * [`app`] — root [`App`](app::App) component; mounts the router.
//! * [`routes`] — [`Route`](routes::Route) enum, [`Home`](routes::home::Home)
//!   and [`Editor`](routes::editor::Editor) components.
//! * [`components`] — shared UI primitives ([`toolbar`](components::toolbar),
//!   [`wgpu_surface`](components::wgpu_surface)).
//! * [`utils`] — utility functions (e.g. [`display_title_from_path`](utils::display_title_from_path)).
//! * [`error`] — [`AppError`](error::AppError) and
//!   [`AppResult`](error::AppResult).
//!
//! Design tokens live in the `loki-theme` workspace crate (`loki_theme::tokens`).

#![warn(missing_docs)]

mod app;
mod components;
mod error;
mod routes;
pub mod utils;

fn main() {
    dioxus::launch(app::App);
}
