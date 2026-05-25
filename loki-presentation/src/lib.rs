// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `loki-presentation` library — Dioxus Native presentation components and routing.

pub mod app;
pub mod new_document;
pub mod recent_documents;
pub mod routes;
pub mod tabs;
pub mod utils;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: android_activity::AndroidApp) {
    blitz_shell::set_android_app(android_app);
    loki_i18n::init();
    dioxus::launch(app::App);
}
