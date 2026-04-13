// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Route definitions for `loki-text`.
//!
//! [`Route`] is the single source of truth for all navigable screens.  It is
//! passed as the type parameter to Dioxus's [`Router`] component and drives
//! client-side URL matching for both desktop window history and mobile
//! in-process routing.

pub mod editor;
pub mod home;

use dioxus::prelude::*;
use editor::Editor;
use home::Home;

/// Top-level application route enum.
///
/// Each variant maps a URL pattern to a Dioxus component.
///
/// * [`Route::Home`] — the home screen with template gallery and recent files.
/// * [`Route::Editor`] — the document editor shell.
///
/// The `path` field on [`Route::Editor`] carries a URL-safe base64-encoded
/// [`loki_file_access::FileAccessToken`] produced by `loki-file-access`.
/// Using the serialised token rather than a raw filesystem path ensures the
/// app works on Android and iOS, where files are identified by capability
/// tokens rather than paths.
#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    /// Home screen: template gallery and recent files list.
    #[route("/")]
    Home {},

    /// Document editor screen.
    ///
    /// `path` is a URL-safe base64 string returned by
    /// [`loki_file_access::FileAccessToken::serialize`].
    #[route("/editor/:path")]
    Editor { path: String },
}
