// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the profile override parser.
//!
//! The parser is the whole of the risk: a diagnostic aid that silently
//! misparses would make a session report a branch as exercised when it ran the
//! default path — the failure this mechanism exists to prevent, reintroduced by
//! the mechanism itself.

use super::{describe, parse, ProfileOverride};
use crate::device_profile::{PointerPrecision, WindowMode};

/// The recipe a Phase 4 session would actually paste.
#[test]
fn a_full_recipe_parses() {
    let o = parse("pointer=coarse,ppi=96,window=fullscreen,dsf=3");
    assert_eq!(o.pointer, Some(PointerPrecision::Coarse));
    assert_eq!(o.css_px_per_inch, Some(96.0));
    assert_eq!(o.window_mode, Some(WindowMode::FullscreenSingle));
    assert_eq!(o.device_scale_factor, Some(3.0));
    assert_eq!(
        o.display().and_then(|d| d.css_px_per_inch),
        Some(96.0),
        "a ppi override must reach consumers as a PhysicalDisplay",
    );
}

/// Fields are independent: forcing one must not disturb the others, or a session
/// testing the pointer path would silently also be testing a forced display.
#[test]
fn one_field_leaves_the_rest_probed() {
    let o = parse("pointer=coarse");
    assert_eq!(o.pointer, Some(PointerPrecision::Coarse));
    assert_eq!(o.css_px_per_inch, None);
    assert_eq!(o.window_mode, None);
    assert_eq!(o.device_scale_factor, None);
}

/// Whitespace and case are what a person actually types.
#[test]
fn spacing_and_case_are_tolerated() {
    let o = parse(" Pointer = Coarse , WINDOW=Windowed ");
    assert_eq!(o.pointer, Some(PointerPrecision::Coarse));
    assert_eq!(o.window_mode, Some(WindowMode::Windowed));
}

/// A typo must not stop the app starting — and must not silently become a
/// *different* valid setting either.
#[test]
fn unknown_keys_and_values_are_ignored_without_inventing_a_setting() {
    let o = parse("pointer=corase,nonsense=1,ppi=abc,dsf=-2");
    assert!(
        o.is_empty(),
        "a wholly mistyped recipe produced {o:?}; ignoring must mean ignoring, \
         not defaulting to something plausible",
    );
}

/// The reason `describe` exists: a run whose override did nothing is
/// indistinguishable from a run with no override, unless the app says which.
#[test]
fn describe_is_silent_only_when_nothing_is_forced() {
    assert_eq!(describe(ProfileOverride::default()), None);
    let text = describe(parse("pointer=coarse,dsf=3")).expect("something was forced");
    assert!(text.contains("Coarse"), "{text}");
    assert!(text.contains('3'), "{text}");
    // A mistyped recipe describes as nothing, which is the honest report: the
    // session did not run the branch it thought it did.
    assert_eq!(describe(parse("pointer=corase")), None);
}

/// A zero or negative physical measurement is a typo, not a device.
#[test]
fn nonsensical_measurements_are_rejected() {
    assert_eq!(parse("ppi=0").css_px_per_inch, None);
    assert_eq!(parse("ppi=-96").css_px_per_inch, None);
    assert_eq!(parse("dsf=0").device_scale_factor, None);
}
