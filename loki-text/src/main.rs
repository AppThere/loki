// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `loki-text` binary entry point.
//!
//! Launches the Dioxus Native application.  All application logic lives in the
//! `loki_text` library crate (`src/lib.rs`).

fn main() {
    loki_i18n::init();
    dioxus::launch(loki_text::app::App);
}
