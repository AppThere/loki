// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The calibration store's behaviour, minus the filesystem.
//!
//! `load`/`save` touch a real per-user path and are deliberately untested here:
//! a test that wrote to the reader's data directory would be a test with a side
//! effect on the machine running it. What is tested is everything the store
//! *decides* — which is where a wrong answer would be silent.

use super::{DisplayCalibrations, DisplayKey};
use crate::display_density::{DensitySource, DisplayDensity};

fn density(ppi: f32) -> DisplayDensity {
    DisplayDensity {
        css_px_per_inch: ppi,
        source: DensitySource::Calibrated,
    }
}

/// A key travels with the panel, so the same panel is the same key.
#[test]
fn a_key_is_derived_from_the_panel_not_its_position() {
    let a = DisplayKey::from_output("eDP-1", 2560, 1600);
    let b = DisplayKey::from_output("eDP-1", 2560, 1600);
    assert_eq!(a, b);
    assert_ne!(a, DisplayKey::from_output("HDMI-1", 2560, 1600));
    assert_ne!(
        a,
        DisplayKey::from_output("eDP-1", 1920, 1200),
        "the same output at a different resolution is a different measurement",
    );
}

/// **Two displays keep two calibrations.** The docked-laptop case, and the whole
/// reason the store is a map — one remembered number would be wrong on whichever
/// panel the reader is not looking at.
#[test]
fn each_display_keeps_its_own_calibration() {
    let mut c = DisplayCalibrations::default();
    let laptop = DisplayKey::from_output("eDP-1", 2560, 1600);
    let external = DisplayKey::from_output("DP-3", 3840, 2160);
    c.set(laptop.clone(), density(110.0));
    c.set(external.clone(), density(163.0));

    assert!((c.get(&laptop).expect("laptop").css_px_per_inch - 110.0).abs() < 0.01);
    assert!((c.get(&external).expect("external").css_px_per_inch - 163.0).abs() < 0.01);
}

/// Re-calibrating replaces, rather than accumulating — a reader who measures
/// again means the second measurement.
#[test]
fn re_calibrating_replaces_the_earlier_measurement() {
    let mut c = DisplayCalibrations::default();
    let k = DisplayKey::unidentified();
    c.set(k.clone(), density(110.0));
    c.set(k.clone(), density(120.0));
    assert!((c.get(&k).expect("stored").css_px_per_inch - 120.0).abs() < 0.01);
}

/// **An unknown display has no calibration, which is what "first use" means.**
/// Both polarities: a stored key reports calibrated, an absent one does not, and
/// the prompt rule turns on exactly this.
#[test]
fn contains_answers_the_first_use_question() {
    let mut c = DisplayCalibrations::default();
    let k = DisplayKey::from_output("eDP-1", 2560, 1600);
    assert!(!c.contains(&k), "never measured");
    c.set(k.clone(), density(110.0));
    assert!(c.contains(&k), "measured");
    assert!(
        !c.contains(&DisplayKey::from_output("DP-3", 3840, 2160)),
        "a different panel is still first use",
    );
}

/// **An implausible value is never stored**, so the file cannot come to hold
/// something the reader would experience as "the calibration did not save".
#[test]
fn an_implausible_calibration_is_not_stored() {
    let mut c = DisplayCalibrations::default();
    let k = DisplayKey::unidentified();
    c.set(k.clone(), density(3.0));
    assert!(!c.contains(&k), "3 ppi is not a screen");
    c.set(k.clone(), density(f32::NAN));
    assert!(!c.contains(&k));
}

/// **And a value that arrived some other way is re-checked on read.** The file
/// is a plain JSON document a reader can edit; trusting it because it was
/// trusted once is how a hand-edited nonsense figure becomes a confidently
/// wrong page size.
#[test]
fn a_stored_value_is_validated_again_when_read() {
    let json = r#"{"entries":{"unidentified":4.0}}"#;
    let c: DisplayCalibrations = serde_json::from_str(json).expect("parses");
    assert_eq!(
        c.get(&DisplayKey::unidentified()),
        None,
        "a nonsense figure in the file must not be believed",
    );

    // The polarity: a believable stored figure *is* returned, so the test above
    // is not passing merely because nothing ever loads.
    let json = r#"{"entries":{"unidentified":110.0}}"#;
    let c: DisplayCalibrations = serde_json::from_str(json).expect("parses");
    assert!(c.contains(&DisplayKey::unidentified()));
}

/// The stored form round-trips, and its source comes back as `Calibrated` —
/// the provenance `note_display_density` needs so a platform reading cannot
/// overwrite a reader's measurement.
#[test]
fn a_stored_calibration_reads_back_as_calibrated() {
    let mut c = DisplayCalibrations::default();
    let k = DisplayKey::unidentified();
    c.set(k.clone(), density(110.0));
    let json = serde_json::to_string(&c).expect("serialises");
    let back: DisplayCalibrations = serde_json::from_str(&json).expect("parses");
    assert_eq!(
        back.get(&k).expect("stored").source,
        DensitySource::Calibrated
    );
}
