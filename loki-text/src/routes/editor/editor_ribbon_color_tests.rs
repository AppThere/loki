// SPDX-License-Identifier: Apache-2.0

//! The highlight swatch list, which pairs `loki-doc-model`'s colour table with
//! this crate's aria keys **by position**.

use super::{HIGHLIGHT_ARIA, highlight_swatches, recent_swatches};
use loki_doc_model::style::props::HIGHLIGHT_RGB;

/// **The two lists are the same length.** They are zipped, so a shorter aria
/// list silently drops swatches off the end of the palette — the dark greys and
/// black, which is exactly the tail nobody notices is missing.
#[test]
fn highlight_swatches_are_complete() {
    assert_eq!(
        HIGHLIGHT_ARIA.len(),
        HIGHLIGHT_RGB.len(),
        "the aria list and the colour table must pair one-to-one",
    );
    assert_eq!(highlight_swatches().len(), HIGHLIGHT_RGB.len());
}

/// **Every swatch value is a hex, not a variant name** (Spec 08 T5.3). The
/// picker compares the current colour against swatch values and `apply_highlight`
/// resolves them, so a variant name here would show no swatch as active and
/// route through `HighlightColor::from_hex`, which would reject it.
#[test]
fn every_swatch_carries_a_hex() {
    for s in highlight_swatches() {
        assert!(
            s.value.starts_with('#') && s.value.len() == 7,
            "swatch value is not a hex: {}",
            s.value
        );
        assert_eq!(s.fill, s.value, "a swatch paints the colour it applies");
    }
}

/// The swatches are in the colour table's order, which is what makes the
/// positional pairing with the aria keys a pairing rather than a coincidence.
#[test]
fn the_swatches_follow_the_colour_table() {
    let values: Vec<String> = highlight_swatches().into_iter().map(|s| s.value).collect();
    let expected: Vec<String> = HIGHLIGHT_RGB
        .iter()
        .map(|(_, hex)| (*hex).to_string())
        .collect();
    assert_eq!(values, expected);
}

/// A stored colour paints itself. This replaced a value-to-fill lookup that
/// returned "transparent" for anything it did not recognise — which was every
/// document colour in the Highlight picker.
#[test]
fn a_stored_colour_is_its_own_fill() {
    let swatches = recent_swatches(&["#C0392B".to_string(), "#FFFF00".to_string()]);
    assert_eq!(swatches.len(), 2);
    for s in swatches {
        assert_eq!(s.fill, s.value);
        assert_ne!(s.fill, "transparent");
    }
}
