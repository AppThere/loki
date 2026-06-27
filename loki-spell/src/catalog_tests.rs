// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::Catalog;
use crate::license::LicenseClass;

#[test]
fn builtin_parses_and_has_entries() {
    let catalog = Catalog::builtin().expect("embedded catalog parses");
    assert!(!catalog.entries().is_empty());
}

#[test]
fn bundled_en_is_permissive_with_source() {
    let catalog = Catalog::builtin().unwrap();
    let en = catalog.get("en").expect("en present");
    assert!(en.bundled);
    assert_eq!(en.license_class, LicenseClass::Permissive);
    assert!(
        en.source.is_some(),
        "en is also downloadable for verification"
    );
}

#[test]
fn resolve_walks_fallback_chain() {
    let catalog = Catalog::builtin().unwrap();
    assert_eq!(catalog.resolve("en-US").map(|e| e.tag.as_str()), Some("en"));
    assert_eq!(catalog.resolve("fr_FR").map(|e| e.tag.as_str()), Some("fr"));
    assert!(catalog.resolve("xx-YY").is_none());
}

#[test]
fn downloadable_languages_are_not_bundled() {
    let catalog = Catalog::builtin().unwrap();
    for tag in ["de", "es", "fr"] {
        let e = catalog.get(tag).expect("present");
        assert!(!e.bundled, "{tag} must not be bundled");
        assert!(e.license_class.requires_consent(), "{tag} is copyleft-ish");
    }
}

#[test]
fn rejects_bundled_nonpermissive_entry() {
    // A manifest that marks a GPL dictionary as bundled violates policy.
    let bad = r#"{
        "entries": [{
            "tag": "de",
            "english_name": "German",
            "native_name": "Deutsch",
            "license_spdx": "GPL-3.0",
            "license_class": "copyleft",
            "bundled": true,
            "source": null
        }]
    }"#;
    assert!(Catalog::from_json(bad).is_err());
}
