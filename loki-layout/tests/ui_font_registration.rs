// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regression lock for the launch-time UI-font registration (deferred plan
//! 4c.4, topic `font`): every app registers `loki_fonts::ui_font_blobs()`
//! into the renderer's Parley `FontContext` at startup (desktop via
//! `dioxus::native::Config::with_fonts`, Android via `android_main!`), so
//! ribbon/chrome labels using `FONT_FAMILY_UI` must resolve "Atkinson
//! Hyperlegible Next" — never fall back to `system-ui`. This test performs
//! the same registration and asserts each bundled family name is queryable.

use loki_layout::FontResources;

#[test]
fn ui_font_blobs_register_every_bundled_family() {
    let mut fonts = FontResources::new();
    for blob in loki_fonts::ui_font_blobs() {
        fonts.register_font(blob);
    }
    for family in [
        "Atkinson Hyperlegible Next",
        "Carlito",
        "Caladea",
        "Arimo",
        "Cousine",
        "Tinos",
        "Gelasio",
    ] {
        assert!(
            fonts.font_cx.collection.family_id(family).is_some(),
            "{family} must resolve after ui_font_blobs registration \
             (UI labels would silently fall back to system-ui)"
        );
    }
}
