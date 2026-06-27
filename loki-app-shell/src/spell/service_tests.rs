// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::SpellService;

#[test]
fn bootstrap_enables_bundled_english() {
    let svc = SpellService::bootstrap().expect("bootstrap");
    assert!(svc.is_enabled());
    assert_eq!(svc.language(), "en");
    assert!(svc.is_correct("hello"));
    assert!(!svc.is_correct("teh"));
    assert!(svc.is_available_offline("en-US"), "bundled en covers en-US");
}

#[test]
fn snapshot_follows_enabled_flag() {
    let svc = SpellService::bootstrap().unwrap();
    let snap = svc.snapshot().expect("enabled → snapshot");
    assert_eq!(snap.generation, 1);
    assert!(!snap.checker.is_correct("teh"));

    svc.set_enabled(false);
    assert!(svc.snapshot().is_none(), "disabled → no snapshot");
}

#[test]
fn check_reports_misspelling_ranges() {
    let svc = SpellService::bootstrap().unwrap();
    let misspellings = svc.check("hello teh world");
    assert_eq!(misspellings.len(), 1);
    assert_eq!(misspellings[0].word, "teh");
}

#[test]
fn catalog_resolution_and_offline_availability() {
    let svc = SpellService::bootstrap().unwrap();
    assert_eq!(svc.resolve_locale("fr-FR").as_deref(), Some("fr"));
    // French is downloadable but not bundled/installed, so not offline-ready.
    assert!(!svc.is_available_offline("fr"));
    // The catalog is non-empty (en + downloadable languages).
    assert!(svc.available().iter().any(|e| e.tag == "en"));
}

#[test]
fn activating_missing_language_errors() {
    let svc = SpellService::bootstrap().unwrap();
    assert!(svc.activate_language("fr").is_err(), "fr not installed");
    // Bundled language always activates.
    assert!(svc.activate_language("en").is_ok());
}
