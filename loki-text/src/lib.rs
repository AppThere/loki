// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

// SAFETY: `unsafe { loki_file_access::init_android(...) }` is required for the
// Android NativeActivity entry point; there is no safe alternative in the
// current android-activity / loki-file-access API.
// TODO(safe): remove when loki-file-access exposes a safe init wrapper.

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
// COMPAT(android-16): On Android 16 (API 36) ANativeActivity_onCreate fires
// twice in rapid succession, spawning two concurrent android_main threads.
// A static OnceLock would also block legitimate activity-recreation relaunches
// within the same process (process reuse), so use a Mutex<bool> "is-running"
// flag instead: set on entry, cleared on exit, so concurrent duplicates are
// rejected while sequential re-entries (activity destroyed → recreated) succeed.
static ANDROID_MAIN_RUNNING: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: android_activity::AndroidApp) {
    {
        let mut running = ANDROID_MAIN_RUNNING
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if *running {
            // Concurrent duplicate invocation on Android 16 — discard it.
            return;
        }
        *running = true;
    }
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("LOKI")
            .with_max_level(log::LevelFilter::Debug),
    );
    log::info!("android_main: start");
    // init_android is a no-op kept for API compatibility; the Application
    // context used by all JNI calls comes from ndk_context, which
    // android-activity initialises before android_main is called.
    unsafe { loki_file_access::init_android(std::ptr::null_mut()) };
    let (top, bottom) = loki_file_access::query_insets_dp();
    log::info!("android_main: safe area insets top={top} bottom={bottom}");
    appthere_ui::set_safe_area_insets(appthere_ui::SafeAreaInsets {
        top,
        bottom,
        ..Default::default()
    });
    // Store the internal data path before android_app is moved, so that
    // recent_documents can persist to a writable location on Android.
    if let Some(data_path) = android_app.internal_data_path() {
        crate::recent_documents::set_android_data_dir(data_path);
    }
    blitz_shell::set_android_app(android_app);
    log::info!("android_main: i18n init");
    loki_i18n::init();
    log::info!("android_main: launching dioxus");
    dioxus::launch(app::App);
    log::info!("android_main: dioxus exited");
    // Clear the running flag so a subsequent activity-recreation relaunch
    // (in the same process) is allowed to proceed.
    *ANDROID_MAIN_RUNNING
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = false;
}
