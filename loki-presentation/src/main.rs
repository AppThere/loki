// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `loki-presentation` binary entry point.
//!
//! Launches the Dioxus Native application. All application logic lives in the
//! `loki_presentation` library crate (`src/lib.rs`).

#[cfg(not(target_os = "android"))]
fn main() {
    loki_i18n::init();
    dioxus::launch(loki_presentation::app::App);
}

#[cfg(target_os = "android")]
#[android_activity::main]
fn android_main(_app: android_activity::AndroidApp) {
    loki_i18n::init();
    dioxus::launch(loki_presentation::app::App);
}
