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
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("LOKI")
            .with_max_level(log::LevelFilter::Debug),
    );
    // SAFETY: activity_as_ptr() is a GlobalRef owned by android_app, which
    // blitz_shell::set_android_app keeps alive for the process lifetime.
    unsafe { loki_file_access::init_android(android_app.activity_as_ptr()) };
    let (top, bottom) = loki_file_access::query_insets_dp();
    appthere_ui::set_safe_area_insets(appthere_ui::SafeAreaInsets {
        top,
        bottom,
        ..Default::default()
    });
    blitz_shell::set_android_app(android_app);
    loki_i18n::init();
    dioxus::launch(app::App);
}
