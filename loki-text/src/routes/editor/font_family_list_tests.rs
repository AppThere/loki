// SPDX-License-Identifier: Apache-2.0

//! Tests for the font picker's list model.

use super::*;

fn device() -> Vec<String> {
    [
        "Helvetica Neue",
        "Charter",
        "Iowan Old Style",
        // A bundled face that is *also* installed locally.
        "Tinos",
        "arimo",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

/// Bundled faces come first and carry the family they replace; that label is
/// the answer to "which one is Arial?" without a second lookup.
#[test]
fn bundled_faces_lead_and_name_what_they_replace() {
    let s = font_sections(&device(), "");
    assert!(!s.bundled.is_empty());
    for row in &s.bundled {
        assert!(row.bundled);
        assert!(
            row.substitutes_for.is_some(),
            "{} has no substitution label",
            row.name
        );
    }
}

/// A bundled face installed on the device must appear once, in the bundled
/// section — the section with the stronger guarantee. Two entries for one
/// family is the defect this dedup exists to prevent.
#[test]
fn a_locally_installed_bundled_face_is_not_listed_twice() {
    let s = font_sections(&device(), "");
    assert!(
        s.bundled.iter().any(|r| r.name == "Tinos"),
        "Tinos is bundled"
    );
    for row in &s.device {
        assert!(
            !row.name.eq_ignore_ascii_case("Tinos"),
            "Tinos duplicated into the device section"
        );
        assert!(
            !row.name.eq_ignore_ascii_case("Arimo"),
            "a differently-cased bundled face still duplicated"
        );
    }
}

/// Typing a proprietary name is how a user looks for its stand-in. Returning
/// nothing for `arial` would push them onto a device face their reader may not
/// have.
#[test]
fn searching_for_a_proprietary_name_finds_its_bundled_substitute() {
    let s = font_sections(&device(), "arial");
    assert!(s.bundled.iter().any(|r| r.name == "Arimo"));

    let s = font_sections(&device(), "Times New Roman");
    assert!(s.bundled.iter().any(|r| r.name == "Tinos"));

    let s = font_sections(&device(), "cambria");
    assert!(s.bundled.iter().any(|r| r.name == "Caladea"));
}

/// The filter is a substring match on the family name, case-insensitively, and
/// must actually exclude non-matches — a filter that returns everything is not
/// a filter.
#[test]
fn the_filter_is_case_insensitive_and_excludes_non_matches() {
    for query in ["char", "CHAR", "Charter"] {
        let s = font_sections(&device(), query);
        assert!(
            s.device.iter().any(|r| r.name == "Charter"),
            "{query} did not match Charter"
        );
        assert!(
            !s.device.iter().any(|r| r.name == "Helvetica Neue"),
            "{query} should not match Helvetica Neue"
        );
    }
}

/// An empty query shows everything — the picker's resting state.
#[test]
fn an_empty_query_shows_every_family() {
    let s = font_sections(&device(), "");
    assert_eq!(s.bundled.len(), loki_fonts::bundled_families().len());
    // Five device names, two of which are bundled and deduped away.
    assert_eq!(s.device.len(), 3);
    assert!(!s.is_empty());
}

/// A query matching nothing must report empty in *both* sections, so the picker
/// can show a "no matches" state instead of a blank list.
#[test]
fn a_query_matching_nothing_is_empty_in_both_sections() {
    let s = font_sections(&device(), "zzzzz-no-such-face");
    assert!(s.bundled.is_empty());
    assert!(s.device.is_empty());
    assert!(s.is_empty());
}

/// Device faces are sorted so the list is scannable; the input order is not.
#[test]
fn device_faces_are_sorted_by_name() {
    let s = font_sections(&device(), "");
    let names: Vec<&str> = s.device.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Charter", "Helvetica Neue", "Iowan Old Style"]);
}

/// A device face carries no substitution label — it stands for itself, and a
/// label would imply a guarantee Loki cannot make for it.
#[test]
fn device_faces_carry_no_substitution_label() {
    let s = font_sections(&device(), "");
    for row in &s.device {
        assert!(!row.bundled);
        assert_eq!(row.substitutes_for, None, "{}", row.name);
    }
}

/// A device with no fonts at all still offers the bundled set — the faces that
/// are always available.
#[test]
fn the_bundled_section_survives_an_empty_device() {
    let s = font_sections(&[], "");
    assert!(s.device.is_empty());
    assert_eq!(s.bundled.len(), loki_fonts::bundled_families().len());
}
