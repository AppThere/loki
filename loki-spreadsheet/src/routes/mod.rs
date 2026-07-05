// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Route definitions for `loki-spreadsheet`.

pub mod editor;
pub mod home;
mod home_util;
pub mod shell;

use dioxus::prelude::*;
use editor::Editor;
use home::Home;
use shell::Shell;

/// Top-level application route enum.
#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(Shell)]
    /// Home screen: template gallery and recent files list.
    #[route("/")]
    Home {},

    /// Document editor screen.
    #[route("/editor/:path")]
    Editor { path: String },
}
