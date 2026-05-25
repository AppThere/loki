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
pub mod new_document;
pub mod recent_documents;
pub mod routes;
pub mod tabs;
pub mod utils;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("LOKI")
            .with_max_level(log::LevelFilter::Debug),
    );
    log::info!("android_main: start");
    // Store the NativeActivity Java object so loki-file-access can call
    // startActivityForResult on the Activity (not Application).
    // SAFETY: activity_as_ptr() is a GlobalRef owned by android_app.
    // blitz_shell::set_android_app below keeps android_app alive for the
    // duration of the process, so the pointer remains valid.
    unsafe { loki_file_access::init_android(android_app.activity_as_ptr()) };
    blitz_shell::set_android_app(android_app);
    log::info!("android_main: i18n init");
    loki_i18n::init();
    log::info!("android_main: launching dioxus");
    dioxus::launch(app::App);
    log::info!("android_main: dioxus exited");
}
