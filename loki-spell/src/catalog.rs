// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The dictionary catalog: which languages exist, their licenses, and where to
//! download them from.
//!
//! The catalog is data-driven (an embedded JSON manifest, [`Catalog::builtin`]),
//! so it can be extended without code changes and the same schema can later be
//! served from the network for over-the-air catalog updates.

use serde::{Deserialize, Serialize};

use crate::error::{SpellError, SpellResult};
use crate::license::LicenseClass;
use crate::locale::fallback_chain;

/// The embedded catalog manifest.
const BUILTIN_CATALOG: &str = include_str!("../assets/catalog.json");

/// Download location and integrity metadata for one dictionary's two files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictionarySource {
    /// URL of the Hunspell affix (`.aff`) file.
    pub aff_url: String,
    /// Expected lowercase hex SHA-256 of the affix file.
    pub aff_sha256: String,
    /// Expected byte length of the affix file.
    pub aff_size: u64,
    /// URL of the Hunspell dictionary (`.dic`) file.
    pub dic_url: String,
    /// Expected lowercase hex SHA-256 of the dictionary file.
    pub dic_sha256: String,
    /// Expected byte length of the dictionary file.
    pub dic_size: u64,
}

/// One catalog entry: a dictionary for a single language tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictionaryEntry {
    /// BCP-47 language tag, e.g. `"en"`, `"pt-BR"`.
    pub tag: String,
    /// English name of the language (reference data, not UI chrome).
    pub english_name: String,
    /// Native name (endonym) of the language (reference data).
    pub native_name: String,
    /// The dictionary's SPDX license expression, verbatim.
    pub license_spdx: String,
    /// Coarse license bucket the bundling/consent policy acts on.
    pub license_class: LicenseClass,
    /// Whether this dictionary is bundled in the application binary.
    ///
    /// The catalog deserialization rejects a bundled entry whose class is not
    /// permissive, enforcing the policy at load time.
    pub bundled: bool,
    /// Where to download the dictionary, if it is downloadable.
    pub source: Option<DictionarySource>,
}

/// The set of available dictionaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    entries: Vec<DictionaryEntry>,
}

impl Catalog {
    /// Parses the catalog embedded in the crate.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::CatalogParse`] if the embedded manifest is invalid
    /// (a build-time bug, exercised by the unit tests).
    pub fn builtin() -> SpellResult<Self> {
        Self::from_json(BUILTIN_CATALOG)
    }

    /// Parses a catalog from a JSON manifest (e.g. a network-fetched update).
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::CatalogParse`] on invalid JSON, or if any entry
    /// marked `bundled` carries a non-permissive license (policy violation).
    pub fn from_json(json: &str) -> SpellResult<Self> {
        let catalog: Catalog =
            serde_json::from_str(json).map_err(|e| SpellError::CatalogParse(e.to_string()))?;
        for entry in &catalog.entries {
            if entry.bundled && !entry.license_class.is_bundleable() {
                return Err(SpellError::CatalogParse(format!(
                    "entry '{}' is marked bundled but its license ({}) is not permissive",
                    entry.tag, entry.license_spdx
                )));
            }
        }
        Ok(catalog)
    }

    /// All catalog entries, in manifest order.
    pub fn entries(&self) -> &[DictionaryEntry] {
        &self.entries
    }

    /// Looks up an entry by exact (case-insensitive) language tag.
    pub fn get(&self, tag: &str) -> Option<&DictionaryEntry> {
        let want = crate::locale::normalize(tag);
        self.entries
            .iter()
            .find(|e| crate::locale::normalize(&e.tag) == want)
    }

    /// Resolves a user locale to the best available entry via the BCP-47
    /// fallback chain (e.g. `"en-US"` → `"en"`).
    pub fn resolve(&self, locale: &str) -> Option<&DictionaryEntry> {
        fallback_chain(locale)
            .into_iter()
            .find_map(|candidate| self.get(&candidate))
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
