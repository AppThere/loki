// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The viewport an overlay is placed against. Split from `geometry.rs` at the
//! 300-line ceiling, on a real seam: this is about the *space*, the rest is about
//! where in it.

use super::Rect;

/// The usable window rect, or `fallback` where the window has not been measured.
///
/// `None` and `(0, 0)` are both "not measured": the first means nobody is
/// publishing, the second that the sensor has not reported yet. Both keep the
/// caller's own viewport rather than clamping into a zero rect, because on the
/// first frame that is the difference between a menu where the consumer asked and
/// a menu at the origin.
/// Exposed rather than private to the host because a consumer's *tests* need the
/// same rule: asserting a menu clears the safe area means asserting it against
/// the viewport the host will actually use, and a test computing that itself is
/// the two-derivations shape (L08-029).
#[must_use]
pub fn usable_viewport(
    window: Option<(f64, f64)>,
    insets: crate::SafeAreaInsets,
    fallback: Rect,
) -> Rect {
    match window {
        Some((w, h)) if w > 0.0 && h > 0.0 => Rect::new(
            insets.left,
            insets.top,
            (w as f32 - insets.left - insets.right).max(0.0),
            (h as f32 - insets.top - insets.bottom).max(0.0),
        ),
        _ => fallback,
    }
}
