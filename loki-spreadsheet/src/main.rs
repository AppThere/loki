// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! `loki-spreadsheet` binary entry point.
//!
//! Launches the Dioxus Native application. All application logic lives in the
//! `loki_spreadsheet` library crate (`src/lib.rs`).

// 1. Import the android entry point macro
#[cfg(target_os = "android")]
use dioxus::desktop::tao::platform::android::activity::android_main;

// 2. Decorate your main function with the attribute macro
#[cfg_attr(target_os = "android", android_main)]
fn main() {
    loki_i18n::init();
    dioxus::launch(loki_spreadsheet::app::App);
}
