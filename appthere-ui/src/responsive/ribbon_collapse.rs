// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Width-driven ribbon collapse cascade (Spec 04 M3 §7).
//!
//! **Decision D3: collapse is width-driven, not tier-driven.** The breakpoint
//! sets defaults, but this engine measures the available width and collapses
//! groups by declared priority until they fit. Per group, the cascade is
//! Full → Condensed → Overflow → (horizontal-scroll floor).
//!
//! # Cascade policy
//!
//! Groups collapse in **priority order** — a lower [`GroupMetrics::priority`]
//! collapses before a higher one (ties break by original left-to-right order).
//! Degradation is graceful: the engine first *condenses* groups (lowest priority
//! first, preserving as much labelled density as possible), and only once every
//! group is condensed does it start *overflowing* whole groups into the "More"
//! menu (again lowest priority first). This keeps the most-used, highest-priority
//! groups fully visible the longest.
//!
//! # Hysteresis
//!
//! The decision is **hysteretic** (like Spec 03's `page_fit`): the strip
//! collapses one step further the instant it overflows, but re-expands a step
//! only when the less-collapsed layout clears the available width by
//! [`RIBBON_COLLAPSE_HYSTERESIS_PX`]. So dragging a window back and forth across
//! a fit threshold does not thrash. The result is idempotent at a fixed width.
//!
//! # Pure and testable
//!
//! All math is in CSS px and takes caller-measured group widths, so the cascade
//! is unit-testable without a Blitz runtime (Spec 03 D1). Wiring the actual
//! per-group width measurement and the overflow-menu UI into `AtRibbon` builds
//! on top of this engine.

use crate::tokens::layout::{RIBBON_COLLAPSE_HYSTERESIS_PX, RIBBON_OVERFLOW_BUTTON_PX};
use crate::tokens::spacing::{SPACE_1, SPACE_2};

/// How a single ribbon group is displayed at the resolved collapse level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupCollapse {
    /// Labelled group, full-size controls, group label visible.
    Full,
    /// Controls pack tighter; the label may drop and low-priority controls may
    /// merge into a dropdown.
    Condensed,
    /// The whole group has moved into the overflow ("More") menu.
    Overflow,
}

/// A group's occupied width (CSS px) in the Full and Condensed states, plus its
/// collapse priority. An overflowed group occupies no strip width (it lives in
/// the "More" menu); the menu button's own width is added once when any group
/// overflows.
///
/// `condensed_px` should be `<= full_px` and, for graceful degradation, at least
/// [`RIBBON_OVERFLOW_BUTTON_PX`] (a group narrower than the "More" chip would not
/// save strip width by overflowing) — real ribbon groups satisfy both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupMetrics {
    /// Higher = kept full longer. Lower-priority groups condense and overflow
    /// first.
    pub priority: u8,
    /// Width occupied in [`GroupCollapse::Full`].
    pub full_px: f32,
    /// Width occupied in [`GroupCollapse::Condensed`].
    pub condensed_px: f32,
}

/// The resolved cascade for one ribbon content strip.
#[derive(Clone, Debug, PartialEq)]
pub struct RibbonCascade {
    /// Per-group display state, in the caller's original group order.
    pub states: Vec<GroupCollapse>,
    /// Number of collapse steps applied (0 = all Full; `2 * groups` = all
    /// overflowed). Carry this back in as `prev_level` next resize for hysteresis.
    pub level: usize,
    /// Whether the overflow ("More") menu button is shown (≥1 group overflowed).
    pub overflow: bool,
    /// Whether even the fully-overflowed strip still exceeds the width — the
    /// horizontal-scroll floor (§7 step 4), never the first resort.
    pub scroll: bool,
}

/// The in-strip layout a group adopts for a given [`GroupCollapse`] state:
/// whether it renders in the strip at all, its horizontal padding and
/// inter-control gap (CSS px), and whether its label row shows.
///
/// Pure so the ribbon group's visual cascade (§7 step 2) is testable without a
/// Blitz runtime; [`AtRibbonGroup`](crate::AtRibbonGroup) applies it directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupLayout {
    /// `false` for [`GroupCollapse::Overflow`] — the group lives in the "More"
    /// menu and paints nothing in the strip.
    pub rendered: bool,
    /// Horizontal padding on each side of the group (CSS px).
    pub pad_px: f32,
    /// Gap between the group's controls (CSS px).
    pub gap_px: f32,
    /// Whether the group's label row shows (dropped when condensed).
    pub show_label: bool,
}

/// The strip layout for a group in `collapse`, given whether it declares a label.
///
/// Condensed reclaims width by dropping the label and tightening padding/gap; it
/// never shrinks the buttons themselves, so touch targets are preserved.
#[must_use]
pub fn group_layout(collapse: GroupCollapse, has_label: bool) -> GroupLayout {
    match collapse {
        GroupCollapse::Full => GroupLayout {
            rendered: true,
            pad_px: SPACE_2,
            gap_px: 2.0,
            show_label: has_label,
        },
        GroupCollapse::Condensed => GroupLayout {
            rendered: true,
            pad_px: SPACE_1,
            gap_px: 0.0,
            show_label: false,
        },
        GroupCollapse::Overflow => GroupLayout {
            rendered: false,
            pad_px: 0.0,
            gap_px: 0.0,
            show_label: false,
        },
    }
}

/// Indices of `metrics` in collapse order: ascending priority, ties by original
/// order (a stable sort of the index list).
fn collapse_order(metrics: &[GroupMetrics]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..metrics.len()).collect();
    order.sort_by_key(|&i| (metrics[i].priority, i));
    order
}

/// The per-group states after applying `level` collapse steps. Steps `1..=n`
/// condense groups in collapse order; steps `n+1..=2n` overflow them (a group is
/// already condensed before it overflows).
fn states_at_level(metrics: &[GroupMetrics], order: &[usize], level: usize) -> Vec<GroupCollapse> {
    let n = metrics.len();
    let condense_count = level.min(n);
    let overflow_count = level.saturating_sub(n).min(n);
    let mut states = vec![GroupCollapse::Full; n];
    for (rank, &idx) in order.iter().enumerate() {
        states[idx] = if rank < overflow_count {
            GroupCollapse::Overflow
        } else if rank < condense_count {
            GroupCollapse::Condensed
        } else {
            GroupCollapse::Full
        };
    }
    states
}

/// The width (CSS px) the strip occupies for `states`, including the "More"
/// button once when any group has overflowed.
fn strip_width(metrics: &[GroupMetrics], states: &[GroupCollapse]) -> f32 {
    let mut width = 0.0;
    let mut any_overflow = false;
    for (m, s) in metrics.iter().zip(states) {
        match s {
            GroupCollapse::Full => width += m.full_px,
            GroupCollapse::Condensed => width += m.condensed_px,
            GroupCollapse::Overflow => any_overflow = true,
        }
    }
    if any_overflow {
        width += RIBBON_OVERFLOW_BUTTON_PX;
    }
    width
}

/// Resolves the collapse cascade for `available_px` of strip width, given the
/// previously-resolved `prev_level` (pass `0` on first layout).
///
/// Hysteretic: collapses further the moment the strip overflows, re-expands only
/// when the less-collapsed layout clears `available_px` by
/// [`RIBBON_COLLAPSE_HYSTERESIS_PX`]. An unmeasured width (`<= 0`) holds
/// `prev_level` unchanged (nothing to decide yet).
#[must_use]
pub fn resolve_cascade(
    metrics: &[GroupMetrics],
    available_px: f32,
    prev_level: usize,
) -> RibbonCascade {
    let n = metrics.len();
    let max_level = 2 * n;
    let order = collapse_order(metrics);
    let width_at = |level: usize| strip_width(metrics, &states_at_level(metrics, &order, level));

    let mut level = prev_level.min(max_level);
    if available_px > 0.0 {
        // Collapse further while the strip overflows and steps remain.
        while level < max_level && width_at(level) > available_px {
            level += 1;
        }
        // Re-expand while the next-looser level clears the width by the band.
        while level > 0 && width_at(level - 1) + RIBBON_COLLAPSE_HYSTERESIS_PX <= available_px {
            level -= 1;
        }
    }

    let states = states_at_level(metrics, &order, level);
    let overflow = level > n;
    let scroll = level >= max_level && width_at(max_level) > available_px && available_px > 0.0;
    RibbonCascade {
        states,
        level,
        overflow,
        scroll,
    }
}

#[cfg(test)]
#[path = "ribbon_collapse_tests.rs"]
mod tests;
