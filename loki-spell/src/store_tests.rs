// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::DictionaryStore;
use crate::catalog::DictionaryEntry;
use crate::license::LicenseClass;
use std::path::PathBuf;

fn test_entry(tag: &str, class: LicenseClass) -> DictionaryEntry {
    DictionaryEntry {
        tag: tag.to_string(),
        english_name: "Test".to_string(),
        native_name: "Test".to_string(),
        license_spdx: "MIT".to_string(),
        license_class: class,
        bundled: false,
        source: None,
    }
}

/// A unique, freshly-cleaned temp directory for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("loki-spell-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn install_then_load_round_trips() {
    let root = temp_dir("install-load");
    let store = DictionaryStore::new(&root);
    let entry = test_entry("en", LicenseClass::Permissive);

    assert!(!store.is_installed("en"));
    store
        .install(&entry, b"SET UTF-8\n", b"1\nhello\n")
        .expect("install");
    assert!(store.is_installed("en"));

    let (aff, dic) = store.load("en").expect("load");
    assert_eq!(aff, "SET UTF-8\n");
    assert_eq!(dic, "1\nhello\n");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn load_missing_is_not_installed_error() {
    let root = temp_dir("load-missing");
    let store = DictionaryStore::new(&root);
    assert!(matches!(
        store.load("fr"),
        Err(crate::error::SpellError::NotInstalled(_))
    ));
}

#[test]
fn meta_records_license() {
    let root = temp_dir("meta");
    let store = DictionaryStore::new(&root);
    let entry = test_entry("de", LicenseClass::Copyleft);
    store.install(&entry, b"a", b"1\nx\n").expect("install");

    let meta = store.meta("de").expect("meta");
    assert_eq!(meta.tag, "de");
    assert_eq!(meta.license_class, LicenseClass::Copyleft);
    assert_eq!(meta.license_spdx, "MIT");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn installed_lists_all_and_remove_deletes() {
    let root = temp_dir("installed-list");
    let store = DictionaryStore::new(&root);
    store
        .install(&test_entry("en", LicenseClass::Permissive), b"a", b"1\nx\n")
        .unwrap();
    store
        .install(
            &test_entry("fr", LicenseClass::LesserCopyleft),
            b"a",
            b"1\nx\n",
        )
        .unwrap();

    let mut tags: Vec<String> = store.installed().into_iter().map(|m| m.tag).collect();
    tags.sort();
    assert_eq!(tags, ["en", "fr"]);

    store.remove("fr").expect("remove");
    assert!(!store.is_installed("fr"));
    assert_eq!(store.installed().len(), 1);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn installed_on_absent_root_is_empty() {
    let store = DictionaryStore::new(temp_dir("absent-root"));
    assert!(store.installed().is_empty());
}
