// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Window title, default size, and size persistence for the desktop shell.
//!
//! The window opens at the size the user last left it (persisted via
//! [`loki_app_shell::window_geometry`]) and falls back to a comfortable
//! default instead of winit's tiny built-in one. Live sizes are reported by
//! [`appthere_ui::AtWindowSizeSensor`] (mounted in `App`) and saved through a
//! short debounce so an interactive resize writes once, not per frame.

use loki_app_shell::window_geometry::WindowGeometry;

/// The product name shown in the OS window title bar.
pub const WINDOW_TITLE: &str = "Loki Text";

/// Relative persistence path under the platform data dir.
pub const GEOMETRY_FILE: &str = "AppThere/Loki/window.json";

/// Default logical inner size for a first launch (no persisted geometry).
pub const DEFAULT_GEOMETRY: WindowGeometry = WindowGeometry::new(1280.0, 800.0);

/// The geometry to open the window with: last-persisted, else the default.
pub fn initial_geometry() -> WindowGeometry {
    WindowGeometry::load(GEOMETRY_FILE).unwrap_or(DEFAULT_GEOMETRY)
}

/// Schedules a debounced save of `size` (see
/// [`loki_app_shell::window_geometry::save_debounced`]).
pub fn persist_geometry_debounced(size: (f64, f64)) {
    loki_app_shell::window_geometry::save_debounced(GEOMETRY_FILE, size);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_geometry_is_sane_without_a_persisted_file() {
        // Whatever is (or is not) on disk, the result is a usable size.
        let g = initial_geometry();
        assert!(g.width >= 320.0 && g.height >= 240.0);
    }
}
