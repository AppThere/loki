// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `loki-spreadsheet` binary entry point.
//!
//! Launches the Dioxus Native application. All application logic lives in the
//! `loki_spreadsheet` library crate (`src/lib.rs`).

fn main() {
    loki_i18n::init();
    // Register the bundled UI + metric-compatible fonts directly into the
    // renderer's font collection so they resolve synchronously on every platform,
    // not via the asynchronous `@font-face` `data:` URI fetch (unreliable on
    // Android). See `loki_fonts::ui_font_blobs`.
    dioxus::native::launch_cfg(
        loki_spreadsheet::app::App,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new().with_fonts(loki_fonts::ui_font_blobs()),
        )],
    );
}
