// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Font-substitution suite (Spec 02 §7.3 / D4).
//!
//! Fidelity fixtures reference the metric-compatible font names directly, so
//! rendering diffs never mix with substitution behaviour. This suite is the
//! *other* half: it references the **original proprietary names** and asserts
//! the substitution engine (`FontResources::resolve_font_name`) maps each to
//! its bundled metric-compatible equivalent, records the substitution (which
//! is what drives the editor's font-substitution warning), and actually has
//! the substitute face available for layout.

use loki_layout::FontResources;

/// Every proprietary family with a bundled metric-compatible substitute.
const SUBSTITUTION_TABLE: &[(&str, &str)] = &[
    ("Calibri", "Carlito"),
    ("Cambria", "Caladea"),
    ("Arial", "Arimo"),
    ("Courier New", "Cousine"),
    ("Times New Roman", "Tinos"),
    ("Georgia", "Gelasio"),
];

/// Each proprietary name must resolve to its metric-compatible substitute
/// (unless the proprietary face is genuinely installed on this machine, in
/// which case resolving to itself is correct and no substitution is recorded).
#[test]
fn proprietary_names_resolve_to_bundled_substitutes() {
    let mut fonts = FontResources::new();
    for (requested, substitute) in SUBSTITUTION_TABLE {
        let resolved = fonts.resolve_font_name(requested);
        if resolved == *requested {
            // The real face is installed here — legitimate, but then no
            // substitution may be recorded for it.
            assert_eq!(
                fonts.substitutions.get(*requested),
                None,
                "{requested} resolved to itself but a substitution was recorded"
            );
        } else {
            assert_eq!(
                &resolved, substitute,
                "{requested} must substitute to {substitute}, got {resolved}"
            );
        }
    }
}

/// A substitution must be *recorded* — the editor's warning banner reads
/// `FontResources::substitutions`, so a silent swap would hide the change
/// from the user.
#[test]
fn substitutions_are_recorded_for_the_warning_banner() {
    let mut fonts = FontResources::new();
    for (requested, substitute) in SUBSTITUTION_TABLE {
        let resolved = fonts.resolve_font_name(requested);
        if resolved != *requested {
            assert_eq!(
                fonts.substitutions.get(*requested),
                Some(&Some((*substitute).to_string())),
                "{requested} → {substitute} substitution must be recorded"
            );
        }
    }
}

/// The resolved family must be layout-usable: present in the Fontique
/// collection with at least one face.
#[test]
fn resolved_families_are_available_for_layout() {
    let mut fonts = FontResources::new();
    for (requested, _) in SUBSTITUTION_TABLE {
        let resolved = fonts.resolve_font_name(requested);
        assert!(
            fonts.font_cx.collection.family_id(&resolved).is_some(),
            "{requested} resolved to {resolved}, which is not in the collection"
        );
    }
}

/// A family with no known substitute resolves to itself and is recorded as
/// unresolved (`None`), which the warning banner reports as "missing".
#[test]
fn unknown_family_is_recorded_as_unresolved() {
    let mut fonts = FontResources::new();
    let resolved = fonts.resolve_font_name("Loki Nonexistent Face 2026");
    assert_eq!(resolved, "Loki Nonexistent Face 2026");
    assert_eq!(
        fonts.substitutions.get("Loki Nonexistent Face 2026"),
        Some(&None),
        "an unresolvable family must be recorded as missing"
    );
}
