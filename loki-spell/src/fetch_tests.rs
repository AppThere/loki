// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{install_dictionary, verify, DictionaryFetcher};
use crate::catalog::{DictionaryEntry, DictionarySource};
use crate::error::{SpellError, SpellResult};
use crate::license::{Consent, LicenseClass};
use crate::store::DictionaryStore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

/// A fetcher backed by an in-memory URL → bytes map.
struct MapFetcher {
    files: HashMap<String, Vec<u8>>,
}

impl DictionaryFetcher for MapFetcher {
    fn fetch(&self, url: &str) -> SpellResult<Vec<u8>> {
        self.files
            .get(url)
            .cloned()
            .ok_or_else(|| SpellError::Download(format!("no such url: {url}")))
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Builds an entry whose checksums match `aff`/`dic`, plus a fetcher that serves
/// exactly those bytes at the entry's URLs.
fn fixture(
    tag: &str,
    class: LicenseClass,
    aff: &[u8],
    dic: &[u8],
) -> (DictionaryEntry, MapFetcher) {
    let aff_url = format!("https://test/{tag}.aff");
    let dic_url = format!("https://test/{tag}.dic");
    let entry = DictionaryEntry {
        tag: tag.to_string(),
        english_name: "Test".to_string(),
        native_name: "Test".to_string(),
        license_spdx: "TEST".to_string(),
        license_class: class,
        bundled: false,
        source: Some(DictionarySource {
            aff_url: aff_url.clone(),
            aff_sha256: hex(&Sha256::digest(aff)),
            aff_size: aff.len() as u64,
            dic_url: dic_url.clone(),
            dic_sha256: hex(&Sha256::digest(dic)),
            dic_size: dic.len() as u64,
        }),
    };
    let mut files = HashMap::new();
    files.insert(aff_url, aff.to_vec());
    files.insert(dic_url, dic.to_vec());
    (entry, MapFetcher { files })
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("loki-spell-fetch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn permissive_installs_without_consent() {
    let root = temp_dir("permissive");
    let store = DictionaryStore::new(&root);
    let (entry, fetcher) = fixture("en", LicenseClass::Permissive, b"SET UTF-8\n", b"1\nhi\n");

    install_dictionary(&store, &entry, &fetcher, Consent::Denied).expect("installs");
    assert!(store.is_installed("en"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn copyleft_blocked_without_consent() {
    let root = temp_dir("copyleft-block");
    let store = DictionaryStore::new(&root);
    let (entry, fetcher) = fixture("de", LicenseClass::Copyleft, b"a", b"1\nx\n");

    let err = install_dictionary(&store, &entry, &fetcher, Consent::Denied);
    assert!(matches!(err, Err(SpellError::ConsentRequired { .. })));
    assert!(!store.is_installed("de"), "nothing installed when refused");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn copyleft_installs_with_consent() {
    let root = temp_dir("copyleft-ok");
    let store = DictionaryStore::new(&root);
    let (entry, fetcher) = fixture("de", LicenseClass::Copyleft, b"a", b"1\nx\n");

    install_dictionary(&store, &entry, &fetcher, Consent::Granted).expect("installs");
    assert!(store.is_installed("de"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn corrupt_download_is_rejected() {
    let root = temp_dir("corrupt");
    let store = DictionaryStore::new(&root);
    // Build a valid fixture, then poison the served dic bytes.
    let (entry, mut fetcher) = fixture("en", LicenseClass::Permissive, b"SET UTF-8\n", b"1\nhi\n");
    let dic_url = entry.source.as_ref().unwrap().dic_url.clone();
    fetcher.files.insert(dic_url, b"tampered".to_vec());

    let err = install_dictionary(&store, &entry, &fetcher, Consent::Granted);
    assert!(matches!(err, Err(SpellError::Integrity { .. })));
    assert!(
        !store.is_installed("en"),
        "corrupt content is not installed"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn verify_detects_size_and_hash_mismatch() {
    let good = b"hello";
    let good_hash = hex(&Sha256::digest(good));
    assert!(verify("x", "dic", good, &good_hash, good.len() as u64).is_ok());
    // Wrong size.
    assert!(verify("x", "dic", good, &good_hash, 999).is_err());
    // Wrong hash.
    assert!(verify("x", "dic", good, &"00".repeat(32), good.len() as u64).is_err());
}
