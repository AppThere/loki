// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What the field accepts — and, as much as a unit test can say it, what it
//! deliberately no longer decides.

use super::parse_measurement;

/// The readings a person actually types, including the comma decimal separator
/// most of Europe writes.
#[test]
fn a_ruler_reading_parses_however_it_is_written() {
    for (text, want) in [
        ("85.6", 85.6_f32),
        ("85,6", 85.6),
        ("  85.6  ", 85.6),
        ("86", 86.0),
    ] {
        let got = parse_measurement(text).unwrap_or_else(|| panic!("{text:?} rejected"));
        assert!((got - want).abs() < 0.001, "{text:?} gave {got}");
    }
}

/// The other polarity: text that is not a measurement.
#[test]
fn what_is_not_a_length_is_not_a_measurement() {
    for text in ["", "   ", "abc", "-5", "0", "85.6mm", "NaN", "inf"] {
        assert_eq!(parse_measurement(text), None, "{text:?} was accepted");
    }
}

/// **The centimetre mistake parses, and that is the point.**
///
/// `8.56` is what a reader types when they read centimetres off the ruler —
/// the mistake the dialog's own prose warns about. It is a perfectly good
/// number, so the parse must accept it and the *caller* must refuse it
/// (`calibrated_css_ppi` does: it implies a 10x density ratio).
///
/// This is the case the previous split got wrong. The dialog treated "positive"
/// as "believable", cleared its rejection notice, and handed the value to a
/// caller whose `let … else { return }` dropped it — so Apply did nothing and
/// said nothing. Asserted here as a parse that *succeeds*, because a test
/// asserting it fails would be re-introducing the second copy of the rule.
#[test]
fn the_centimetre_mistake_is_a_number_and_not_this_modules_verdict() {
    assert!(
        parse_measurement("8.56").is_some(),
        "the parse must not grow an opinion about plausibility — that decision \
         belongs to the caller, which is why on_measured returns a bool",
    );
}
