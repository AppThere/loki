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
    blitz_shell::set_android_app(android_app);
    log::info!("android_main: i18n init");
    loki_i18n::init();
    log::info!("android_main: launching dioxus");
    dioxus::launch(app::App);
    log::info!("android_main: dioxus exited");
}
