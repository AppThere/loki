// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The shared [`SpellService`] held in each app's context.

use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use loki_spell::{
    Catalog, Consent, DictionaryEntry, DictionaryFetcher, DictionaryStore, InstalledMeta,
    Misspelling, SpellChecker, SpellError, SpellResult, bundled, install_dictionary, locale,
};

/// An immutable snapshot of the active checker, handed to the layout engine.
///
/// `generation` increments whenever the active dictionary changes; hosts fold it
/// into their layout cache key so cached squiggles are reused only while the
/// dictionary is unchanged.
#[derive(Clone)]
pub struct SpellSnapshot {
    /// The shared, thread-safe checker.
    pub checker: Arc<SpellChecker>,
    /// Monotonic generation of the active dictionary.
    pub generation: u64,
}

/// Suite-shared spell-check service. Cheap to clone (an `Arc` handle), so it can
/// be provided into a Dioxus context and read from any component.
#[derive(Clone)]
pub struct SpellService {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    checker: Arc<SpellChecker>,
    generation: u64,
    language: String,
    enabled: bool,
    store: DictionaryStore,
    catalog: Catalog,
}

impl SpellService {
    /// Boots the service with the bundled English dictionary enabled.
    ///
    /// # Errors
    ///
    /// Only fails if the embedded dictionary or catalog is corrupt (a build
    /// defect), so callers can treat an error as fatal-or-disable.
    pub fn bootstrap() -> SpellResult<Self> {
        let checker = Arc::new(SpellChecker::bundled()?);
        let catalog = Catalog::builtin()?;
        let root = super::dictionaries_dir().unwrap_or_else(|| "dictionaries".into());
        let inner = Inner {
            checker,
            generation: 1,
            language: bundled::BUNDLED_TAG.to_string(),
            enabled: true,
            store: DictionaryStore::new(root),
            catalog,
        };
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    // ── Query (hot path) ──────────────────────────────────────────────────────

    /// The active checker + generation, or `None` when spell checking is off.
    pub fn snapshot(&self) -> Option<SpellSnapshot> {
        let inner = self.read();
        inner.enabled.then(|| SpellSnapshot {
            checker: Arc::clone(&inner.checker),
            generation: inner.generation,
        })
    }

    /// Whether `word` is spelled correctly in the active dictionary.
    pub fn is_correct(&self, word: &str) -> bool {
        self.read().checker.is_correct(word)
    }

    /// Ranked correction suggestions for `word`.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        self.read().checker.suggest(word)
    }

    /// Misspelled words (with byte ranges) in `text`.
    pub fn check(&self, text: &str) -> Vec<Misspelling> {
        self.read().checker.check_text(text)
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    /// BCP-47 tag of the active dictionary.
    pub fn language(&self) -> String {
        self.read().language.clone()
    }

    /// Whether spell checking is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.read().enabled
    }

    /// Turns spell checking on or off (off makes [`Self::snapshot`] return
    /// `None`, so layout stops emitting squiggles).
    pub fn set_enabled(&self, enabled: bool) {
        self.write().enabled = enabled;
    }

    /// Adds `word` to the personal dictionary and bumps the generation.
    ///
    /// The change is visible immediately through the shared checker; the
    /// generation bump invalidates the layout's paragraph cache so the squiggle
    /// on that word clears on the next relayout. After calling this, refresh the
    /// host's layout-spell state from [`Self::snapshot`] and request a relayout.
    pub fn add_word(&self, word: &str) {
        let mut inner = self.write();
        inner.checker.add_word(word);
        inner.generation += 1;
    }

    /// Ignores `word` for this session and bumps the generation (see
    /// [`Self::add_word`] for the relayout note).
    pub fn ignore_word(&self, word: &str) {
        let mut inner = self.write();
        inner.checker.ignore_word(word);
        inner.generation += 1;
    }

    // ── Catalog / store ───────────────────────────────────────────────────────

    /// All catalog entries (for a language picker).
    pub fn available(&self) -> Vec<DictionaryEntry> {
        self.read().catalog.entries().to_vec()
    }

    /// Metadata for every installed (downloaded) dictionary.
    pub fn installed(&self) -> Vec<InstalledMeta> {
        self.read().store.installed()
    }

    /// Resolves a host locale (e.g. `"en-US"`) to a catalog tag via the BCP-47
    /// fallback chain, or `None` if no dictionary covers it.
    pub fn resolve_locale(&self, host_locale: &str) -> Option<String> {
        self.read()
            .catalog
            .resolve(host_locale)
            .map(|e| e.tag.clone())
    }

    /// Whether a dictionary covering `tag` is ready to activate without
    /// downloading — the bundled language or an installed one, matched through
    /// the BCP-47 fallback chain (so `"en-US"` is covered by bundled `"en"`).
    pub fn is_available_offline(&self, tag: &str) -> bool {
        let inner = self.read();
        locale::fallback_chain(tag)
            .into_iter()
            .any(|cand| cand == bundled::BUNDLED_TAG || inner.store.is_installed(&cand))
    }

    // ── Activation / installation ─────────────────────────────────────────────

    /// Switches the active dictionary to an already-available `tag` (bundled or
    /// installed), bumping the generation so hosts re-lay-out.
    ///
    /// This parses the dictionary (tens of milliseconds for a large language);
    /// callers should run it on a worker thread.
    ///
    /// # Errors
    ///
    /// [`SpellError::NotInstalled`] if `tag` is neither bundled nor installed, or
    /// a parse error if the stored files are corrupt.
    pub fn activate_language(&self, tag: &str) -> SpellResult<()> {
        let checker = if locale::normalize(tag) == bundled::BUNDLED_TAG {
            SpellChecker::bundled()?
        } else {
            let (aff, dic) = self.read().store.load(tag)?;
            SpellChecker::new(&aff, &dic)?
        };
        let mut inner = self.write();
        inner.checker = Arc::new(checker);
        inner.generation += 1;
        inner.language = locale::normalize(tag);
        Ok(())
    }

    /// Downloads, verifies, installs, and activates the dictionary for `tag`.
    ///
    /// Enforces the license [`Consent`] gate and SHA-256 integrity (in
    /// `loki-spell`). Run on a worker thread — it performs blocking network I/O.
    ///
    /// # Errors
    ///
    /// [`SpellError::NoSource`] if `tag` is not in the catalog, plus any consent,
    /// download, integrity, or store error.
    pub fn install_and_activate(
        &self,
        tag: &str,
        consent: Consent,
        fetcher: &dyn DictionaryFetcher,
    ) -> SpellResult<()> {
        let entry = self
            .read()
            .catalog
            .get(tag)
            .cloned()
            .ok_or_else(|| SpellError::NoSource(tag.to_string()))?;
        // Scope the read guard so it is released before `activate_language` writes.
        {
            let inner = self.read();
            install_dictionary(&inner.store, &entry, fetcher, consent)?;
        }
        self.activate_language(tag)
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
