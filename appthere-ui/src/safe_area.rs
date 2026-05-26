// SPDX-License-Identifier: Apache-2.0

//! Safe-area insets for edge-to-edge display on mobile platforms.
//!
//! On Android and iOS, system bars (status bar, navigation bar) draw over the
//! application window. Call [`set_safe_area_insets`] from the platform's entry
//! point (e.g. `android_main`) with values queried from the OS, then read them
//! in the root component via [`use_safe_area`] to apply padding so content is
//! not obscured.
//!
//! On desktop platforms the global is never set, so [`use_safe_area`] returns
//! all-zero insets and the root component padding is effectively a no-op.

use std::sync::OnceLock;

/// Platform system-bar insets in density-independent pixels (CSS px / Android dp).
///
/// All values default to `0.0` so the type is safe to use on platforms where
/// edge-to-edge is not applicable (Windows, macOS, Linux).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SafeAreaInsets {
    /// Height of the status bar (top system bar).
    pub top: f32,
    /// Height of the navigation bar (bottom system bar, or gesture strip).
    pub bottom: f32,
    /// Width of any left system decoration (rare; non-zero on some landscape layouts).
    pub left: f32,
    /// Width of any right system decoration (rare; non-zero on some landscape layouts).
    pub right: f32,
}

static INSETS: OnceLock<SafeAreaInsets> = OnceLock::new();

/// Store the platform safe-area insets.
///
/// Must be called before [`dioxus::launch`] so the values are visible to the
/// first component render. Subsequent calls are silently ignored (the OS values
/// do not change after the window is created on the supported platforms).
pub fn set_safe_area_insets(insets: SafeAreaInsets) {
    // OnceLock::set returns Err if already initialised; that is intentional.
    let _ = INSETS.set(insets);
}

/// Return the stored safe-area insets, defaulting to all-zero if never set.
///
/// Call this inside a Dioxus component to apply the insets as padding on the
/// root container so system bars do not obscure application content.
pub fn use_safe_area() -> SafeAreaInsets {
    INSETS.get().copied().unwrap_or_default()
}
