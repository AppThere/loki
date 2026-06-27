// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Dictionary download orchestration.
//!
//! `loki-spell` deliberately carries no HTTP stack: the application owns the
//! network (it already has a TLS client and proxy config). The caller provides a
//! [`DictionaryFetcher`]; this module drives the policy gate, byte fetch,
//! integrity verification, and install so those invariants live in one place
//! rather than being re-implemented per call site.

use sha2::{Digest, Sha256};

use crate::catalog::DictionaryEntry;
use crate::error::{SpellError, SpellResult};
use crate::license::Consent;
use crate::store::DictionaryStore;

/// Transport for downloading dictionary files, supplied by the caller.
///
/// Implementations perform the actual HTTPS GET and return the response body, or
/// a [`SpellError::Download`] carrying the transport error message.
pub trait DictionaryFetcher {
    /// Fetches the full body at `url`.
    fn fetch(&self, url: &str) -> SpellResult<Vec<u8>>;
}

/// Downloads and installs the dictionary described by `entry`.
///
/// Order of operations:
/// 1. **Consent gate** — a consent-requiring (copyleft) entry is refused with
///    [`SpellError::ConsentRequired`] unless [`Consent::Granted`] is passed.
/// 2. **Fetch + verify** — each file is downloaded and checked against the
///    catalog's size and SHA-256; a mismatch yields [`SpellError::Integrity`]
///    and nothing is written.
/// 3. **Install** — verified bytes are committed to the `store`.
///
/// # Errors
///
/// See the steps above, plus [`SpellError::NoSource`] if the entry is not
/// downloadable and [`SpellError::Download`]/[`SpellError::Io`] from the
/// transport and store.
pub fn install_dictionary(
    store: &DictionaryStore,
    entry: &DictionaryEntry,
    fetcher: &dyn DictionaryFetcher,
    consent: Consent,
) -> SpellResult<()> {
    if !consent.satisfies(entry.license_class) {
        return Err(SpellError::ConsentRequired {
            tag: entry.tag.clone(),
            license: entry.license_spdx.clone(),
        });
    }
    let source = entry
        .source
        .as_ref()
        .ok_or_else(|| SpellError::NoSource(entry.tag.clone()))?;

    let aff = fetch_verified(
        fetcher,
        &entry.tag,
        "aff",
        &source.aff_url,
        &source.aff_sha256,
        source.aff_size,
    )?;
    let dic = fetch_verified(
        fetcher,
        &entry.tag,
        "dic",
        &source.dic_url,
        &source.dic_sha256,
        source.dic_size,
    )?;
    store.install(entry, &aff, &dic)
}

fn fetch_verified(
    fetcher: &dyn DictionaryFetcher,
    tag: &str,
    file: &str,
    url: &str,
    expected_sha256: &str,
    expected_size: u64,
) -> SpellResult<Vec<u8>> {
    let bytes = fetcher.fetch(url)?;
    verify(tag, file, &bytes, expected_sha256, expected_size)?;
    Ok(bytes)
}

/// Verifies downloaded `bytes` against the expected size and SHA-256.
///
/// # Errors
///
/// Returns [`SpellError::Integrity`] on any mismatch.
pub fn verify(
    tag: &str,
    file: &str,
    bytes: &[u8],
    expected_sha256: &str,
    expected_size: u64,
) -> SpellResult<()> {
    if bytes.len() as u64 != expected_size {
        return Err(SpellError::Integrity {
            tag: tag.to_string(),
            file: file.to_string(),
            detail: format!("expected {expected_size} bytes, got {}", bytes.len()),
        });
    }
    let digest = Sha256::digest(bytes);
    let actual = hex_lower(&digest);
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        return Err(SpellError::Integrity {
            tag: tag.to_string(),
            file: file.to_string(),
            detail: format!("expected sha256 {expected_sha256}, got {actual}"),
        });
    }
    Ok(())
}

/// Lower-case hex encoding of a byte slice (avoids a hex crate dependency).
fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
