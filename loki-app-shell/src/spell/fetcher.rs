// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Blocking HTTPS transport for dictionary downloads.

use loki_spell::{DictionaryFetcher, SpellError, SpellResult};

/// A [`DictionaryFetcher`] backed by a blocking `reqwest` client (rustls).
///
/// Blocking by design: dictionary installs run on a dedicated worker thread (the
/// apps never call this from the async UI loop), and `loki-spell`'s
/// `install_dictionary` drives the fetch synchronously while it verifies each
/// file's SHA-256 before writing it to the store.
pub struct ReqwestFetcher {
    client: reqwest::blocking::Client,
}

impl ReqwestFetcher {
    /// Builds a fetcher. Errors only if the TLS backend fails to initialise.
    pub fn new() -> SpellResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("loki-app-shell/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| SpellError::Download(e.to_string()))?;
        Ok(Self { client })
    }
}

impl DictionaryFetcher for ReqwestFetcher {
    fn fetch(&self, url: &str) -> SpellResult<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|e| SpellError::Download(e.to_string()))?;
        let bytes = response
            .bytes()
            .map_err(|e| SpellError::Download(e.to_string()))?;
        Ok(bytes.to_vec())
    }
}
