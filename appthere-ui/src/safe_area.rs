// SPDX-License-Identifier: Apache-2.0

//! Safe-area insets for edge-to-edge display on mobile platforms.
//!
//! On Android and iOS, system bars (status bar, navigation bar) and display
//! cutouts draw over the application window. The platform entry point seeds an
//! initial value with [`set_safe_area_insets`]; the root component reads it via
//! [`use_safe_area`] and applies padding so content is not obscured.
//!
//! The insets are **not** fixed for the lifetime of the app: on Android they
//! change with orientation (in landscape the navigation bar / cutout move to a
//! side). Call [`update_safe_area_insets`] from within the Dioxus runtime (e.g.
//! a resize handler) to push new values — readers of [`use_safe_area`]
//! re-render so the padding follows the current orientation.
//!
//! On desktop platforms nothing ever updates the value, so [`use_safe_area`]
//! returns all-zero insets and the root padding is effectively a no-op.

use std::sync::RwLock;

use dioxus::prelude::*;

/// Platform system-bar / cutout insets in density-independent pixels (CSS px /
/// Android dp).
///
/// All values default to `0.0` so the type is safe to use on platforms where
/// edge-to-edge is not applicable (Windows, macOS, Linux).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SafeAreaInsets {
    /// Height of the top system inset (status bar / top cutout).
    pub top: f32,
    /// Height of the bottom system inset (navigation bar / gesture strip).
    pub bottom: f32,
    /// Width of the left system inset (non-zero in some landscape layouts).
    pub left: f32,
    /// Width of the right system inset (non-zero in some landscape layouts).
    pub right: f32,
}

/// Current insets. A plain `RwLock` (not a signal) so the platform entry point
/// can seed it before the Dioxus runtime exists.
static INSETS: RwLock<SafeAreaInsets> = RwLock::new(SafeAreaInsets {
    top: 0.0,
    bottom: 0.0,
    left: 0.0,
    right: 0.0,
});

/// Reactivity trigger: bumped by [`update_safe_area_insets`] so components that
/// read [`use_safe_area`] re-render. Separate from `INSETS` because a global
/// signal cannot be written before the runtime is initialised.
static VERSION: GlobalSignal<u64> = Signal::global(|| 0);

/// Seed the platform safe-area insets, typically from the platform entry point
/// (e.g. `android_main`) before `dioxus::launch`. Does not notify — the first
/// render reads the stored value.
pub fn set_safe_area_insets(insets: SafeAreaInsets) {
    if let Ok(mut w) = INSETS.write() {
        *w = insets;
    }
}

/// Update the safe-area insets at runtime (e.g. on an orientation change) and
/// re-render every [`use_safe_area`] reader. No-op when the value is unchanged,
/// so it is safe to call on every resize tick.
///
/// Must be called from within the Dioxus runtime (it writes a global signal).
pub fn update_safe_area_insets(insets: SafeAreaInsets) {
    let unchanged = INSETS.read().map(|cur| *cur == insets).unwrap_or(false);
    if unchanged {
        return;
    }
    if let Ok(mut w) = INSETS.write() {
        *w = insets;
    }
    *VERSION.write() += 1;
}

/// Return the current safe-area insets, defaulting to all-zero if never set.
///
/// Call this inside a Dioxus component to apply the insets as padding on the
/// root container. Subscribes to [`update_safe_area_insets`], so the component
/// re-renders when the insets change (orientation change).
pub fn use_safe_area() -> SafeAreaInsets {
    // Subscribe to updates.
    let _ = VERSION();
    INSETS.read().map(|i| *i).unwrap_or_default()
}
