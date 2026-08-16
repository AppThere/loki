// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super`] (`font.rs`) — extracted per the file-ceiling
//! convention (CLAUDE.md technique 1).

use super::*;

#[test]
fn test_font_resolution_fallback() {
    let mut r = FontResources::new();

    // Aptos should be missing (not installed in typical environments)
    let resolved = r.resolve_font_name("Aptos");
    assert_eq!(resolved, "Aptos");
    assert!(r.substitutions.contains_key("Aptos"));
    assert_eq!(r.substitutions.get("Aptos"), Some(&None));

    // Test standard substitute: Calibri -> Carlito (if Carlito is missing, it should resolve to Calibri and track as None)
    let resolved = r.resolve_font_name("Calibri");
    if r.font_cx.collection.family_id("Carlito").is_some() {
        assert_eq!(resolved, "Carlito");
        assert_eq!(
            r.substitutions.get("Calibri"),
            Some(&Some("Carlito".to_string()))
        );
    } else {
        assert_eq!(resolved, "Calibri");
        assert_eq!(r.substitutions.get("Calibri"), Some(&None));
    }

    // Test case-insensitive behavior: calibri -> Carlito or calibri
    let resolved = r.resolve_font_name("calibri");
    if r.font_cx.collection.family_id("Carlito").is_some() {
        assert_eq!(resolved, "Carlito");
        assert_eq!(
            r.substitutions.get("calibri"),
            Some(&Some("Carlito".to_string()))
        );
    } else {
        assert_eq!(resolved, "calibri");
        assert_eq!(r.substitutions.get("calibri"), Some(&None));
    }

    // "Calibri Light" (Word's default heading face) must resolve to the same
    // metric-compatible substitute as Calibri — otherwise headings/titles
    // fall back to a wider face and wrap differently from Word.
    let resolved = r.resolve_font_name("Calibri Light");
    if r.font_cx.collection.family_id("Carlito").is_some() {
        assert_eq!(resolved, "Carlito", "Calibri Light must map to Carlito");
    }
}

// Regression guard: the embedded metric-compatible faces must be available on
// every platform (not gated to Android), so headless/CI/PDF-export builds can
// register them. Re-gating to `target_os = "android"` would fail this here.
#[test]
fn fallback_font_blobs_embedded_on_all_targets() {
    assert!(
        !loki_fonts::fallback_font_blobs().is_empty(),
        "metric-compatible fallback faces must be embedded on this target"
    );
}

// Regression guard for the actual rendering bug: resolving a font with a known
// metric-compatible substitute must yield a family that is *actually present*
// in the collection. Before lazy fallback registration, "Calibri" resolved to
// itself but Carlito was absent on desktop → Parley fell back to a digit-less
// font, so list markers and Calibri text rendered `.notdef`.
#[test]
fn substituted_family_is_actually_available() {
    let mut r = FontResources::new();
    for requested in ["Calibri", "Arial", "Times New Roman", "Cambria", "Georgia"] {
        let resolved = r.resolve_font_name(requested);
        assert!(
            r.font_cx.collection.family_id(resolved.as_str()).is_some(),
            "{requested:?} resolved to {resolved:?}, which is not available in the collection",
        );
    }
}

#[test]
fn run_recording_reports_only_fonts_touched_since_begin() {
    let mut r = FontResources::new();

    // A missing font resolved during the run is recorded.
    r.begin_substitution_run();
    r.resolve_font_name("Aptos");
    let run = r.take_substitution_run();
    assert_eq!(run.get("Aptos"), Some(&None), "missing font recorded");

    // Inversion: a run that touches nothing reports nothing — even though the
    // process-lifetime memo still holds the earlier entry.
    r.begin_substitution_run();
    let run = r.take_substitution_run();
    assert!(
        run.is_empty(),
        "an untouched run must report no substitutions"
    );
    assert!(
        r.substitutions.contains_key("Aptos"),
        "while the resolve memo keeps its entry"
    );
}

#[test]
fn run_recording_re_records_memo_hits() {
    // The property per-document reporting depends on: a font already in the
    // resolve memo (from an earlier document) must still be recorded when a
    // later run requests it again.
    let mut r = FontResources::new();
    r.begin_substitution_run();
    r.resolve_font_name("Courier New");
    let first = r.take_substitution_run();

    r.begin_substitution_run();
    r.resolve_font_name("Courier New"); // memo hit this time
    let second = r.take_substitution_run();
    assert_eq!(
        first, second,
        "a memo hit must record the same outcome as the original resolve"
    );
    assert!(!second.is_empty());
}

#[test]
fn a_bundled_family_requested_by_name_always_resolves() {
    // A template (or a document authored in Loki) names a bundled face
    // directly. On a machine where it is not installed system-wide, the
    // embedded faces must be registered and the name answered as-is — not
    // recorded as missing (`None`), which is what happened when only the
    // proprietary-name substitute arms triggered registration.
    let mut r = FontResources::new();
    r.begin_substitution_run();
    let resolved = r.resolve_font_name("Courier Prime");
    assert_eq!(resolved, "Courier Prime");
    assert_ne!(
        r.substitutions.get("Courier Prime"),
        Some(&None),
        "a bundled family must never be recorded as missing"
    );

    // Control (guard inversion): a non-bundled absent family still records
    // as missing — the bundled arm must not swallow genuine misses.
    let unresolved = r.resolve_font_name("Definitely Not A Font");
    assert_eq!(unresolved, "Definitely Not A Font");
    assert_eq!(r.substitutions.get("Definitely Not A Font"), Some(&None));
}

#[test]
fn bare_courier_substitutes_to_courier_prime() {
    // Screenplays from other tools commonly name plain "Courier".
    let mut r = FontResources::new();
    r.begin_substitution_run();
    assert_eq!(r.resolve_font_name("Courier"), "Courier Prime");
}
