// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `loki-text` binary entry point.
//!
//! Launches the Dioxus Native application.  All application logic lives in the
//! `loki_text` library crate (`src/lib.rs`).

/// Installs a log sink so `tracing` macros reach a terminal.
///
/// Without this every `tracing::debug!`/`warn!` in the workspace is a silent
/// no-op on desktop — the macros compile and run and their output goes nowhere.
/// That is how Spec 08's texture-residency diagnostic came to be written,
/// committed, and relied on by a screen-test procedure while being impossible to
/// observe. An instrument nobody can read is not an instrument.
///
/// Off by default (`RUST_LOG` unset means errors only), so ordinary runs are
/// quiet. To watch the texture budget:
///
/// ```text
/// RUST_LOG=loki_renderer=debug LOKI_TEXTURE_BUDGET_MB=24 cargo run -p loki-text --bin loki-text-desktop
/// ```
fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));
    // `try_init` rather than `init`, so a failure cannot take the app down over
    // logging. The desktop binary installs this once, so the only realistic
    // failure is an environment that already has a global subscriber — worth
    // saying out loud, because the symptom otherwise is a diagnostic that prints
    // nothing and reads as "the code path never ran".
    if let Err(err) = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init()
    {
        eprintln!("loki-text: logging unavailable ({err}); RUST_LOG will have no effect");
    }
}

fn main() {
    init_logging();
    // §13: files handed to us by the OS (file-manager "Open with", argv).
    loki_text::startup_files::stash_from_args(std::env::args().skip(1));
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
