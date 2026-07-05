// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Zoom stepping shared by the suite's status-bar zoom badges.

/// The zoom step after `current`, for the status bar's zoom badge — cycles
/// 50 → 75 → 100 → 125 → 150 → 200 → back to 50 (unknown values snap to 100).
#[must_use]
pub fn next_zoom(current: u32) -> u32 {
    match current {
        50 => 75,
        75 => 100,
        100 => 125,
        125 => 150,
        150 => 200,
        200 => 50,
        _ => 100,
    }
}
