// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `loki-text` binary entry point.
//!
//! Launches the Dioxus Native application.  All application logic lives in the
//! `loki_text` library crate (`src/lib.rs`).

fn main() {
    loki_i18n::init();
    // Register the bundled UI + metric-compatible fonts directly into the
    // renderer's font collection at startup. This is the robust, cross-platform
    // path: the families resolve synchronously, without depending on the
    // asynchronous `@font-face` `data:` URI fetch (which is unreliable on
    // Android). See `loki_fonts::ui_font_blobs`.
    dioxus::native::launch_cfg(
        loki_text::app::App,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new().with_fonts(loki_fonts::ui_font_blobs()),
        )],
    );
}
