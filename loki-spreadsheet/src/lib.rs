// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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

#[cfg(target_os = "android")]
// COMPAT(android-16): On Android 16 (API 36) ANativeActivity_onCreate fires
// twice in rapid succession, spawning two concurrent android_main threads.
// A Mutex<bool> "is-running" flag rejects concurrent duplicates while allowing
// sequential re-entries after activity-recreation (process reuse).
static ANDROID_MAIN_RUNNING: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: android_activity::AndroidApp) {
    {
        let mut running = ANDROID_MAIN_RUNNING
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if *running {
            return;
        }
        *running = true;
    }
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("LOKI-SHEET")
            .with_max_level(log::LevelFilter::Debug),
    );
    // Route panic messages to logcat. The default panic hook writes to
    // stderr, which Android discards — without this, any Rust panic (e.g.
    // during GPU renderer init) is indistinguishable from a native crash.
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {info}");
    }));
    log::info!("android_main: start");
    // SAFETY: activity_as_ptr() is a GlobalRef owned by android_app, which
    // blitz_shell::set_android_app keeps alive for the process lifetime.
    unsafe { loki_file_access::init_android(android_app.activity_as_ptr()) };
    let (top, bottom) = loki_file_access::query_insets_dp();
    appthere_ui::set_safe_area_insets(appthere_ui::SafeAreaInsets {
        top,
        bottom,
        ..Default::default()
    });
    if let Some(data_path) = android_app.internal_data_path() {
        crate::recent_documents::set_android_data_dir(data_path);
    }
    blitz_shell::set_android_app(android_app);
    log::info!("android_main: i18n init");
    loki_i18n::init();
    log::info!("android_main: launching dioxus");
    // Pre-register bundled UI + metric fonts synchronously (see
    // `loki_fonts::ui_font_blobs`) so the UI font resolves on Android without
    // the asynchronous `@font-face` `data:` URI fetch.
    dioxus::native::launch_cfg(
        app::App,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new().with_fonts(loki_fonts::ui_font_blobs()),
        )],
    );
    log::info!("android_main: dioxus exited");
    *ANDROID_MAIN_RUNNING
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = false;
}
