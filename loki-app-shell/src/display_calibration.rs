// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Remembering a reader's display calibration, per display (Spec 08 T5.5).
//!
//! # Per display, because a calibration is about a panel and not about a person
//!
//! T5.5 says "per-display user calibration", and the reason is concrete: a laptop
//! docked to an external monitor has two panels with different densities, and one
//! remembered number would be wrong on whichever the reader is not looking at.
//! Storing it against a [`DisplayKey`] makes moving between them restore the
//! right measurement instead of silently applying the other one.
//!
//! # The key is content-derived, not positional
//!
//! Monitor *order* changes when a cable is replugged, so an index would silently
//! reattach a calibration to a different panel — the same identity hazard T4.2's
//! recent menu had with `Signal<Option<usize>>`, where the consequence was
//! deleting the wrong file. Here the consequence is a page printed at the wrong
//! size, which is quieter and therefore worse. The key is the output's reported
//! name and pixel geometry, which travel with the panel.
//!
//! An unidentifiable display gets [`DisplayKey::unidentified`], and that is
//! deliberately a *real* key rather than a refusal to store: a reader on a
//! platform with no query at all still gets their one calibration remembered,
//! and the cost of the collision — two unidentifiable panels sharing an entry —
//! is a wrong density they can re-calibrate, versus a prompt on every launch.
//!
//! # The paragraph above was a description, not a behaviour, until the Phase 5
//! close audit
//!
//! Every read and every write used `unidentified`: [`DisplayKey::from_output`]
//! existed, was tested, and was called by nothing. A map keyed by a constant is
//! a map with one key, so the docked-laptop case this module opens with did not
//! work — and it looked finished, because the key type and its tests were both
//! correct. That is L08-050's shape: a decision with tests and no caller reads
//! as more done than one with neither. The producer is now
//! `loki_text::device_probe::current_display_key`, which both call sites use.
//!
//! Measured on X11, both directions: calibrating at 1280x900 stores
//! `screen:1280x900` and a relaunch restores it; the same binary on a
//! 1600x1000 display reports `screen:1600x1000` and restores **nothing** — which
//! is the half that would have passed for a constant key.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::display_density::{DensitySource, DisplayDensity, plausible_ppi};

/// Relative path under the platform data directory.
///
/// Shared by the whole suite rather than per app: the reader calibrated their
/// *screen*, and asking again in Calc because they first measured in Text would
/// be asking the same question twice.
pub const CALIBRATION_FILE: &str = "AppThere/display-calibration.json";

/// Identifies a display across sessions.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct DisplayKey(pub String);

impl DisplayKey {
    /// A key from what a platform query reported.
    #[must_use]
    pub fn from_output(name: &str, width_px: u32, height_px: u32) -> Self {
        Self(format!("{name}:{width_px}x{height_px}"))
    }

    /// The key used when the platform reports nothing identifying.
    #[must_use]
    pub fn unidentified() -> Self {
        Self("unidentified".to_string())
    }
}

/// Calibrations the reader has made, by display.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct DisplayCalibrations {
    /// CSS pixels per inch, by display key.
    ///
    /// A `BTreeMap` so the file's key order is stable: a map that reshuffles on
    /// every write makes every save a diff, which matters if a reader ever syncs
    /// their config directory.
    entries: BTreeMap<DisplayKey, f32>,
}

impl DisplayCalibrations {
    /// Load, or an empty set on any error — a missing or corrupt file must not
    /// stop the app, and the consequence is one calibration prompt.
    #[must_use]
    pub fn load() -> Self {
        calibration_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// The calibration for `key`, if one was stored and is still believable.
    ///
    /// Re-validated on read, not only on write. A file can be hand-edited, and a
    /// stored value that has become implausible must not be trusted just because
    /// it was trusted once — the check is cheap and the failure it prevents is a
    /// silently wrong physical size.
    #[must_use]
    pub fn get(&self, key: &DisplayKey) -> Option<DisplayDensity> {
        let ppi = *self.entries.get(key)?;
        plausible_ppi(ppi).then_some(DisplayDensity {
            css_px_per_inch: ppi,
            source: DensitySource::Calibrated,
        })
    }

    /// Records a calibration, replacing any earlier one for that display.
    ///
    /// Ignores an implausible value rather than storing it, so the file cannot
    /// come to hold something [`Self::get`] will refuse — a state that would read
    /// as "the calibration did not save".
    pub fn set(&mut self, key: DisplayKey, density: DisplayDensity) {
        if plausible_ppi(density.css_px_per_inch) {
            self.entries.insert(key, density.css_px_per_inch);
        }
    }

    /// Whether this display has been calibrated — the "first use" question
    /// D-04's prompt rule turns on.
    #[must_use]
    pub fn contains(&self, key: &DisplayKey) -> bool {
        self.get(key).is_some()
    }

    /// Persist.
    ///
    /// A failure does not take the app down — a read-only or full disk is not
    /// worth crashing over — but it is **reported** rather than discarded. The
    /// consequence of a silent failure is that the reader is asked to measure
    /// their screen again next launch, which presents as the calibration being
    /// broken rather than as the disk being full.
    pub fn save(&self) {
        let Some(path) = calibration_path() else {
            tracing::warn!("no data directory: display calibration cannot be saved");
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(?err, ?parent, "could not create the calibration directory");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(err) = std::fs::write(&path, json) {
                    tracing::warn!(?err, ?path, "could not save the display calibration");
                }
            }
            Err(err) => tracing::warn!(?err, "could not serialise the display calibration"),
        }
    }
}

/// Where the calibration file lives on this platform.
///
/// Two `cfg`-selected functions rather than one with an early `return` and an
/// `#[allow(unreachable_code)]`: the allow suppresses a warning that is telling
/// the truth on one target, and splitting says the same thing without asking the
/// compiler to stop noticing.
#[cfg(target_os = "android")]
fn calibration_path() -> Option<PathBuf> {
    crate::recent_documents::android_data_dir().map(|d| d.join(CALIBRATION_FILE))
}

#[cfg(not(target_os = "android"))]
fn calibration_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(CALIBRATION_FILE))
}

#[cfg(test)]
#[path = "display_calibration_tests.rs"]
mod tests;
