// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `loki-spreadsheet` binary entry point.
//!
//! Launches the Dioxus Native application. All application logic lives in the
//! `loki_spreadsheet` library crate (`src/lib.rs`).

fn main() {
    loki_i18n::init();
    dioxus::launch(loki_spreadsheet::app::App);
}
