// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared Parley font and layout context.

use std::collections::HashMap;
use std::sync::Arc;

use crate::color::LayoutColor;
use crate::para_cache::ParaCache;

/// Shared font and layout context.
///
/// Construct once per application (or once per document if memory is
/// constrained). Both [`parley::FontContext`] and
/// [`parley::LayoutContext`] are Parley types that amortize allocation
/// across many layouts.
pub struct FontResources {
    /// Parley font database: discovers and caches system fonts.
    pub font_cx: parley::FontContext,
    /// Parley shaping scratch space: reused across layout calls.
    pub layout_cx: parley::LayoutContext<LayoutColor>,
    /// Font data cache: maps raw Parley font-data pointer → shared Arc.
    ///
    /// Parley hands back a `&[u8]` slice per glyph run pointing into its
    /// internal storage. Without this cache, `layout_paragraph` would copy
    /// the entire font file into a fresh `Arc<Vec<u8>>` for every glyph run
    /// (often millions of bytes × thousands of runs = most of the render time).
    /// Keying by the slice's base pointer (cast to `u64`) ensures that glyph
    /// runs from the same Parley-internal blob share a single `Arc`.
    pub(crate) font_data_cache: HashMap<u64, Arc<Vec<u8>>>,
    /// Tracks font availability and substitutions.
    ///
    /// Key: requested font name.
    /// Value: `Some(substitute)` if substituted, or `None` if missing without standard substitute.
    ///
    /// **Process-lifetime memo, not a report.** This doubles as
    /// [`Self::resolve_font_name`]'s cache and is never cleared, so it spans
    /// every document this handle ever laid out. UI reporting must use the
    /// per-run recording ([`Self::begin_substitution_run`] /
    /// [`Self::take_substitution_run`]) instead.
    pub substitutions: HashMap<String, Option<String>>,
    /// Substitutions touched since [`Self::begin_substitution_run`] — the
    /// reportable subset for the layout run in progress.
    run_substitutions: HashMap<String, Option<String>>,
    /// Memoised paragraph layouts, keyed by a hash of every shaping input.
    ///
    /// Lets [`crate::para::layout_paragraph`] skip re-shaping paragraphs that
    /// did not change between layout passes (the common case on a keystroke,
    /// where only one paragraph differs). See [`ParaCache`].
    pub(crate) para_cache: ParaCache,
    /// Whether the embedded metric-compatible fallback faces (Carlito/Caladea/
    /// Arimo/Cousine/Tinos) have been registered. Done lazily, at most once, the
    /// first time a substitute family is requested but found missing.
    fallbacks_registered: bool,
}

impl FontResources {
    /// Creates a new `FontResources`, loading system fonts via Fontique.
    pub fn new() -> Self {
        // Timing under `loki_text::open` so the one-time system-font-scan cost
        // (paid by the first document open in an editor) is visible on-device.
        let started = std::time::Instant::now();
        let mut font_cx = parley::FontContext::new();

        // Dynamically scan and register app-bundled fonts from the assets directory.
        if let Ok(exe_path) = std::env::current_exe()
            && let Some(exe_dir) = exe_path.parent()
        {
            let assets_fonts = exe_dir.join("assets").join("fonts");
            if assets_fonts.is_dir() {
                font_cx.collection.load_fonts_from_paths(vec![assets_fonts]);
            }
        }

        // The bundled metric-compatible fallback faces (Carlito/Caladea/Arimo/
        // Cousine/Tinos) are registered lazily by `resolve_font_name` only when a
        // substitute family is requested but missing — so a properly-installed
        // desktop (where they are found above, or system-wide) never pays the
        // registration, while headless/CI/PDF-export and Android still resolve
        // Calibri/Arial/Times correctly. See `ensure_fallback_fonts_registered`.
        tracing::info!(
            target: "loki_text::open",
            elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
            "FontResources::new: font context built",
        );

        Self {
            font_cx,
            layout_cx: parley::LayoutContext::new(),
            font_data_cache: HashMap::new(),
            substitutions: HashMap::new(),
            run_substitutions: HashMap::new(),
            para_cache: ParaCache::default(),
            fallbacks_registered: false,
        }
    }

    /// Registers the embedded metric-compatible fallback faces into the Fontique
    /// collection, at most once. Called lazily when a substitute family (e.g.
    /// Carlito for Calibri) is requested but not already present, so that
    /// substitution works even when the fonts are not installed system-wide
    /// (headless export, CI, fresh desktop installs, Android).
    fn ensure_fallback_fonts_registered(&mut self) {
        if self.fallbacks_registered {
            return;
        }
        self.fallbacks_registered = true;
        for blob in loki_fonts::fallback_font_blobs() {
            let bytes: Vec<u8> = blob.to_vec();
            self.font_cx
                .collection
                .register_fonts(parley::fontique::Blob::from(bytes), None);
        }
    }

    /// Drops every memoised paragraph layout, freeing the retained
    /// `ParagraphLayout`s. Call when switching documents so the shaping cache
    /// does not retain the previous document's layouts.
    pub fn clear_paragraph_cache(&mut self) {
        self.para_cache.clear();
    }

    /// `(entries, approximate heap bytes)` resident in the paragraph shaping
    /// cache — the usage-audit §15/A1 instrumentation (the cache lives on the
    /// app-root shared resources, so it is the candidate for "memory stays
    /// high after closing every tab"). The byte figure is a floor; see
    /// `ParaCache::stats`.
    #[must_use]
    pub fn para_cache_stats(&self) -> (usize, usize) {
        self.para_cache.stats()
    }

    /// Returns the names of every font family available for layout — the
    /// scanned system fonts plus any bundled or document-embedded faces — sorted
    /// alphabetically (case-insensitive) and de-duplicated.
    ///
    /// Used by the style editor's font picker. Requires `&mut self` because
    /// Fontique populates its family index lazily.
    pub fn available_font_families(&mut self) -> Vec<String> {
        let mut names: Vec<String> = self
            .font_cx
            .collection
            .family_names()
            .map(|s| s.to_string())
            .collect();
        names.sort_by_key(|s| s.to_lowercase());
        names.dedup();
        names
    }

    /// Registers additional font data (e.g. fonts embedded in the document).
    ///
    /// `data` must be valid font bytes (TTF / OTF / TTC). The font is added
    /// to the internal Fontique collection and will be available for future
    /// layout calls.
    pub fn register_font(&mut self, data: Vec<u8>) {
        let blob = parley::fontique::Blob::from(data);
        self.font_cx.collection.register_fonts(blob, None);
    }

    /// Resolves the requested font family name, checking availability and applying standard substitutes if needed.
    ///
    /// If the font is available, returns the original name.
    /// If the font is missing, check standard substitutes and return the substitute if available, recording the change.
    /// If both the font and its substitute are missing (or no substitute exists), returns the original name and records it as missing.
    pub fn resolve_font_name(&mut self, name: &str) -> String {
        // Return cached result if we already processed this font.
        if let Some(sub) = self.substitutions.get(name) {
            self.run_substitutions.insert(name.to_string(), sub.clone());
            return sub.as_ref().cloned().unwrap_or_else(|| name.to_string());
        }

        // Check if the requested font name is available in the collection.
        // Fontique family_id lookup is case-insensitive.
        if self.font_cx.collection.family_id(name).is_some() {
            return name.to_string();
        }

        // A *bundled* family requested by name (a template or a document
        // authored in Loki says "Cousine" or "Courier Prime" directly) must
        // always resolve — that guarantee is why the picker badges bundled
        // faces as substitution-free. Register the embedded faces and answer
        // with the requested name; without this arm a bundled name fell
        // through the substitute table below (which only maps *proprietary*
        // names) and was reported missing on machines where it is not
        // installed system-wide.
        if loki_fonts::is_bundled_family(name) {
            self.ensure_fallback_fonts_registered();
            if self.font_cx.collection.family_id(name).is_some() {
                return name.to_string();
            }
        }

        // Font is not available. Check standard substitutes (case-insensitive).
        let substitute = match name.to_lowercase().as_str() {
            "arial" => Some("Arimo"),
            // Courier Prime matches Courier's metrics (it was drawn as a
            // Courier replacement); Cousine covers the metrically-identical
            // Courier New. Screenplays from other tools commonly name bare
            // "Courier".
            "courier" => Some("Courier Prime"),
            "courier new" => Some("Cousine"),
            "times new roman" => Some("Tinos"),
            // "Calibri Light" is a distinct family in Word (the default heading
            // face) with the same metrics as Calibri, so it maps to the same
            // metric-compatible substitute. Without this arm it falls through to
            // a wider system fallback and headings/titles wrap differently.
            "calibri" | "calibri light" => Some("Carlito"),
            // "Cambria Math" is the math-glyph companion of Cambria; the text
            // face substitute keeps its Latin metrics.
            "cambria" | "cambria math" => Some("Caladea"),
            "georgia" => Some("Gelasio"),
            _ => None,
        };

        if let Some(sub_name) = substitute {
            // If the substitute is not already in the collection (e.g. not
            // installed system-wide), lazily register the embedded faces so the
            // metric-compatible substitution still works.
            if self.font_cx.collection.family_id(sub_name).is_none() {
                self.ensure_fallback_fonts_registered();
            }
            if self.font_cx.collection.family_id(sub_name).is_some() {
                self.substitutions
                    .insert(name.to_string(), Some(sub_name.to_string()));
                self.run_substitutions
                    .insert(name.to_string(), Some(sub_name.to_string()));
                return sub_name.to_string();
            }
        }

        // Nothing available — record as unresolved so the banner reports it.
        self.substitutions.insert(name.to_string(), None);
        self.run_substitutions.insert(name.to_string(), None);
        name.to_string()
    }

    /// Starts a fresh per-run substitution recording. Call at the start of a
    /// layout run, under the same lock the run holds.
    pub fn begin_substitution_run(&mut self) {
        self.run_substitutions.clear();
    }

    /// The substitutions touched since [`Self::begin_substitution_run`].
    ///
    /// A run's recording **under-reports** by itself: paragraph-cache hits and
    /// incrementally reused pages skip font resolution entirely, so their
    /// families are not re-recorded. Callers must accumulate runs into a
    /// per-document set rather than replace it — the accumulator is only
    /// cleared on a fresh document load, whose seed layout re-shapes (and so
    /// re-records) everything.
    pub fn take_substitution_run(&mut self) -> HashMap<String, Option<String>> {
        std::mem::take(&mut self.run_substitutions)
    }
}

impl Default for FontResources {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "font_tests.rs"]
mod tests;
