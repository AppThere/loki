// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! On-disk store for downloaded dictionaries.
//!
//! The store is a directory tree, one subdirectory per installed language tag,
//! each holding `index.aff`, `index.dic`, and a `meta.json` recording the
//! language and license (so the app can show attribution for installed
//! dictionaries). The caller chooses the root path (its platform data dir);
//! `loki-spell` does not decide where application data lives.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::DictionaryEntry;
use crate::error::{SpellError, SpellResult};
use crate::license::LicenseClass;
use crate::locale::normalize;

const AFF_FILE: &str = "index.aff";
const DIC_FILE: &str = "index.dic";
const META_FILE: &str = "meta.json";

/// Recorded metadata for an installed dictionary (written as `meta.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledMeta {
    /// BCP-47 language tag.
    pub tag: String,
    /// English name of the language.
    pub english_name: String,
    /// Native name (endonym) of the language.
    pub native_name: String,
    /// SPDX license expression of the installed dictionary.
    pub license_spdx: String,
    /// Coarse license class.
    pub license_class: LicenseClass,
}

impl InstalledMeta {
    fn from_entry(entry: &DictionaryEntry) -> Self {
        Self {
            tag: entry.tag.clone(),
            english_name: entry.english_name.clone(),
            native_name: entry.native_name.clone(),
            license_spdx: entry.license_spdx.clone(),
            license_class: entry.license_class,
        }
    }
}

/// A filesystem-backed store of installed dictionaries rooted at a directory.
#[derive(Debug, Clone)]
pub struct DictionaryStore {
    root: PathBuf,
}

impl DictionaryStore {
    /// Creates a store rooted at `root` (created lazily on first install).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory holding a given tag's files (normalized tag as dir name).
    fn dir_for(&self, tag: &str) -> PathBuf {
        self.root.join(normalize(tag))
    }

    /// Returns `true` if all three files for `tag` are present.
    pub fn is_installed(&self, tag: &str) -> bool {
        let dir = self.dir_for(tag);
        dir.join(AFF_FILE).is_file()
            && dir.join(DIC_FILE).is_file()
            && dir.join(META_FILE).is_file()
    }

    /// Installs a dictionary's already-fetched bytes for `entry`.
    ///
    /// Writes the affix, dictionary, and `meta.json` atomically enough for local
    /// use (each file written in full; partial state is detectable via
    /// [`Self::is_installed`]). Overwrites any existing install for the tag.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::Io`] if the directory or files cannot be written.
    pub fn install(&self, entry: &DictionaryEntry, aff: &[u8], dic: &[u8]) -> SpellResult<()> {
        let dir = self.dir_for(&entry.tag);
        fs::create_dir_all(&dir).map_err(io)?;
        fs::write(dir.join(AFF_FILE), aff).map_err(io)?;
        fs::write(dir.join(DIC_FILE), dic).map_err(io)?;
        let meta = serde_json::to_vec_pretty(&InstalledMeta::from_entry(entry))
            .map_err(|e| SpellError::Io(e.to_string()))?;
        fs::write(dir.join(META_FILE), meta).map_err(io)?;
        Ok(())
    }

    /// Loads the `(aff, dic)` contents for an installed tag.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::NotInstalled`] if the tag is absent, or
    /// [`SpellError::Io`] if a file cannot be read or is not UTF-8.
    pub fn load(&self, tag: &str) -> SpellResult<(String, String)> {
        if !self.is_installed(tag) {
            return Err(SpellError::NotInstalled(tag.to_string()));
        }
        let dir = self.dir_for(tag);
        let aff = fs::read_to_string(dir.join(AFF_FILE)).map_err(io)?;
        let dic = fs::read_to_string(dir.join(DIC_FILE)).map_err(io)?;
        Ok((aff, dic))
    }

    /// Reads the recorded metadata for an installed tag.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::NotInstalled`] if absent, or [`SpellError::Io`] if
    /// `meta.json` cannot be read or parsed.
    pub fn meta(&self, tag: &str) -> SpellResult<InstalledMeta> {
        if !self.is_installed(tag) {
            return Err(SpellError::NotInstalled(tag.to_string()));
        }
        read_meta(&self.dir_for(tag).join(META_FILE))
    }

    /// Lists metadata for every installed dictionary.
    ///
    /// Subdirectories without readable metadata are skipped rather than failing
    /// the whole scan. Returns an empty list if the store directory is absent.
    pub fn installed(&self) -> Vec<InstalledMeta> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let meta_path = entry.path().join(META_FILE);
            if let Ok(meta) = read_meta(&meta_path) {
                out.push(meta);
            }
        }
        out
    }

    /// Removes an installed dictionary. A no-op if it is not installed.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::Io`] if the directory cannot be removed.
    pub fn remove(&self, tag: &str) -> SpellResult<()> {
        let dir = self.dir_for(tag);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(io)?;
        }
        Ok(())
    }
}

fn read_meta(path: &Path) -> SpellResult<InstalledMeta> {
    let bytes = fs::read(path).map_err(io)?;
    serde_json::from_slice(&bytes).map_err(|e| SpellError::Io(e.to_string()))
}

fn io(e: std::io::Error) -> SpellError {
    SpellError::Io(e.to_string())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
