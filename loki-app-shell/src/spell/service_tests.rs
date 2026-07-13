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
fn add_and_ignore_word_take_effect_and_bump_generation() {
    // Persistence disabled: this test must not write the user profile, and a
    // previously-persisted "zzqq" would break the pre-add assertion.
    let svc = SpellService::bootstrap_with_personal_dict(None).unwrap();
    let gen0 = svc.snapshot().unwrap().generation;

    assert!(!svc.is_correct("zzqq"));
    svc.add_word("zzqq");
    assert!(svc.is_correct("zzqq"));
    let gen1 = svc.snapshot().unwrap().generation;
    assert!(gen1 > gen0, "add_word bumps generation");

    assert!(!svc.is_correct("wuux"));
    svc.ignore_word("WUUX");
    assert!(svc.is_correct("wuux"), "ignore is case-insensitive");
    assert!(svc.snapshot().unwrap().generation > gen1);
}

#[test]
fn activating_missing_language_errors() {
    let svc = SpellService::bootstrap().unwrap();
    assert!(svc.activate_language("fr").is_err(), "fr not installed");
    // Bundled language always activates.
    assert!(svc.activate_language("en").is_ok());
}

/// 5.10: a personal word survives switching the active dictionary — the fresh
/// checker gets the in-memory list replayed into it.
#[test]
fn personal_word_survives_activate_language() {
    let svc = SpellService::bootstrap_with_personal_dict(None).unwrap();
    svc.add_word("zzyzx");
    assert!(svc.is_correct("zzyzx"));

    let gen_before = svc.snapshot().unwrap().generation;
    svc.activate_language("en").expect("bundled activates");
    assert!(
        svc.is_correct("zzyzx"),
        "personal word must survive the dictionary switch"
    );
    assert!(svc.snapshot().unwrap().generation > gen_before);
}

/// 5.10: a personal word persists to disk and is replayed on the next boot.
#[test]
fn personal_word_survives_a_restart() {
    let path = std::env::temp_dir().join("loki-personal-dict-test-service-restart.json");
    let _ = std::fs::remove_file(&path);

    let svc = SpellService::bootstrap_with_personal_dict(Some(path.clone())).unwrap();
    assert!(!svc.is_correct("qwxzz"));
    svc.add_word("qwxzz");
    drop(svc);

    let svc2 = SpellService::bootstrap_with_personal_dict(Some(path.clone())).unwrap();
    assert!(
        svc2.is_correct("qwxzz"),
        "persisted personal word must be replayed on bootstrap"
    );
    let _ = std::fs::remove_file(&path);
}
