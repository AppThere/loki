// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `loki-text` binary entry point.
//!
//! Launches the Dioxus Native application.  All application logic lives in the
//! `loki_text` library crate (`src/lib.rs`).

fn main() {
    loki_i18n::init();
    // Window: proper product title (instead of winit's "Dioxus App") and the
    // last session's inner size (persisted by `window_state`; falls back to a
    // comfortable default rather than winit's tiny built-in size).
    let geometry = loki_text::window_state::initial_geometry();
    let attributes = dioxus::native::WindowAttributes::default()
        .with_title(loki_text::window_state::WINDOW_TITLE)
        .with_inner_size(dioxus::native::LogicalSize::new(
            geometry.width,
            geometry.height,
        ));
    // Register the bundled UI + metric-compatible fonts directly into the
    // renderer's font collection at startup. This is the robust, cross-platform
    // path: the families resolve synchronously, without depending on the
    // asynchronous `@font-face` `data:` URI fetch (which is unreliable on
    // Android). See `loki_fonts::ui_font_blobs`.
    dioxus::native::launch_cfg(
        loki_text::app::App,
        vec![],
        vec![Box::new(
            dioxus::native::Config::new()
                .with_fonts(loki_fonts::ui_font_blobs())
                .with_window_attributes(attributes),
        )],
    );
}
