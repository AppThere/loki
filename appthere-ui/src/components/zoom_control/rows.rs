// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The zoom menu's rows and the walk over them (Spec 08 T5.4).
//!
//! # One table, built from the presets rather than beside them
//!
//! `ZOOM_PRESETS_PERCENT` is already the ladder the stepper walks. The menu shows
//! the same ladder, so it *derives* its preset rows from that array instead of
//! listing them again — the Phase 4 lesson (L08-029) applied before the
//! divergence rather than after it. Adding a preset changes the stepper and the
//! menu together, which is the only way they can be guaranteed to agree.
//!
//! # The computed rows are conditional, and that is what makes the walk non-trivial
//!
//! Fit Width, Fit Page and Actual Size each exist only when the app can actually
//! perform them: the fits need page and viewport measurements the host app holds,
//! and Actual Size needs a physical display density the platform may not report.
//! So the row list is a function of state rather than a constant — the same shape
//! as the spelling menu's suggestions, and the reason the keyboard walks a
//! *computed* `Vec` rather than indexing a fixed array.
//!
//! **Availability is derived from whether a handler exists, not configured
//! beside it** (the `Role::traps_focus` rule). A row the app cannot service is
//! not reachable to be offered, so "a menu row that does nothing" is a state this
//! API cannot express — which matters because the three apps sharing this control
//! do not all have a page geometry to fit to.

use crate::components::zoom::ZOOM_PRESETS_PERCENT;

/// One row of the zoom menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZoomRow {
    /// A preset zoom, by index into [`ZOOM_PRESETS_PERCENT`].
    Preset(usize),
    /// Fit the page width to the viewport.
    FitWidth,
    /// Fit the whole page in the viewport.
    FitPage,
    /// One document inch measures one physical inch.
    ActualSize,
}

impl ZoomRow {
    /// A stable, distinct key — what the pointer and the keyboard agree on.
    ///
    /// Distinct across variants as well as within them: `Preset(0)` and
    /// `FitWidth` sharing a key would highlight two rows at once, and the
    /// collision would only show up for whichever pair happened to collide.
    #[must_use]
    pub fn key(self) -> String {
        match self {
            Self::Preset(i) => format!("zoom-{i}"),
            Self::FitWidth => "fit-width".to_string(),
            Self::FitPage => "fit-page".to_string(),
            Self::ActualSize => "actual-size".to_string(),
        }
    }

    /// The zoom this row selects, when it is a preset.
    #[must_use]
    pub fn preset_percent(self) -> Option<u32> {
        match self {
            Self::Preset(i) => ZOOM_PRESETS_PERCENT.get(i).copied(),
            _ => None,
        }
    }
}

/// Which computed rows this app can service.
///
/// Not `bool` arguments, because three of them in a row is exactly the call site
/// where two get swapped and nothing complains.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ZoomCommands {
    /// The app can compute a fit-width zoom.
    pub fit_width: bool,
    /// The app can compute a fit-page zoom.
    pub fit_page: bool,
    /// This app supports Actual Size at all.
    ///
    /// # Not "a density is known" — that was the r81 mistake
    ///
    /// The row was gated on the platform having reported a density, which reads
    /// as the careful choice and makes T5.5's own fallback **unreachable**: the
    /// calibration prompt is specified to appear "on first use of Actual Size",
    /// so hiding the row until a density exists means there is no first use to
    /// prompt from, on precisely the displays that need it. The capability is
    /// "this app can do Actual Size, given a density it can obtain"; obtaining
    /// one is the handler's problem.
    pub actual_size: bool,
}

/// The menu's rows, in display and keyboard order.
///
/// A row that would do nothing is **not shown**, rather than shown disabled: none
/// of these can be *made* available by the user, so a greyed row is permanent
/// furniture explaining a capability this build does not have.
#[must_use]
pub fn zoom_rows(commands: ZoomCommands) -> Vec<ZoomRow> {
    let mut rows: Vec<ZoomRow> = (0..ZOOM_PRESETS_PERCENT.len())
        .map(ZoomRow::Preset)
        .collect();
    if commands.fit_width {
        rows.push(ZoomRow::FitWidth);
    }
    if commands.fit_page {
        rows.push(ZoomRow::FitPage);
    }
    if commands.actual_size {
        rows.push(ZoomRow::ActualSize);
    }
    rows
}

/// The row after `current`, wrapping — the menu convention, and the opposite of
/// the *stepper's* rule.
///
/// The two live one screen apart and disagree on purpose: wrapping is right when
/// a walk has a visible end (you can see you are at the bottom of a list), and
/// wrong for zoom-in (nothing on screen says the next press will go to 20%).
/// Stated here because "make them consistent" is the plausible-sounding change
/// that breaks one of them.
#[must_use]
pub fn next_zoom_row(rows: &[ZoomRow], current: Option<&str>) -> Option<ZoomRow> {
    step(rows, current, 1)
}

/// The row before `current`, wrapping.
#[must_use]
pub fn prev_zoom_row(rows: &[ZoomRow], current: Option<&str>) -> Option<ZoomRow> {
    step(rows, current, -1)
}

/// Shared walk. A key naming no row lands at an end rather than returning
/// `None`: the hover key survives the menu reopening with a different row set
/// (Actual Size arriving, say), and a first arrow press that did nothing would
/// read as a menu with no keyboard at all.
fn step(rows: &[ZoomRow], current: Option<&str>, delta: isize) -> Option<ZoomRow> {
    if rows.is_empty() {
        return None;
    }
    let len = rows.len() as isize;
    let position = current.and_then(|key| rows.iter().position(|r| r.key() == key));
    let next = match position {
        Some(i) => (i as isize + delta).rem_euclid(len),
        None if delta > 0 => 0,
        None => len - 1,
    };
    rows.get(next as usize).copied()
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
