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
    pub substitutions: HashMap<String, Option<String>>,
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
            return sub.as_ref().cloned().unwrap_or_else(|| name.to_string());
        }

        // Check if the requested font name is available in the collection.
        // Fontique family_id lookup is case-insensitive.
        if self.font_cx.collection.family_id(name).is_some() {
            return name.to_string();
        }

        // Font is not available. Check standard substitutes (case-insensitive).
        let substitute = match name.to_lowercase().as_str() {
            "arial" => Some("Arimo"),
            "courier new" => Some("Cousine"),
            "times new roman" => Some("Tinos"),
            "calibri" => Some("Carlito"),
            "cambria" => Some("Caladea"),
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
                return sub_name.to_string();
            }
        }

        // Nothing available — record as unresolved so the banner reports it.
        self.substitutions.insert(name.to_string(), None);
        name.to_string()
    }
}

impl Default for FontResources {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_resolution_fallback() {
        let mut r = FontResources::new();

        // Aptos should be missing (not installed in typical environments)
        let resolved = r.resolve_font_name("Aptos");
        assert_eq!(resolved, "Aptos");
        assert!(r.substitutions.contains_key("Aptos"));
        assert_eq!(r.substitutions.get("Aptos"), Some(&None));

        // Test standard substitute: Calibri -> Carlito (if Carlito is missing, it should resolve to Calibri and track as None)
        let resolved = r.resolve_font_name("Calibri");
        if r.font_cx.collection.family_id("Carlito").is_some() {
            assert_eq!(resolved, "Carlito");
            assert_eq!(
                r.substitutions.get("Calibri"),
                Some(&Some("Carlito".to_string()))
            );
        } else {
            assert_eq!(resolved, "Calibri");
            assert_eq!(r.substitutions.get("Calibri"), Some(&None));
        }

        // Test case-insensitive behavior: calibri -> Carlito or calibri
        let resolved = r.resolve_font_name("calibri");
        if r.font_cx.collection.family_id("Carlito").is_some() {
            assert_eq!(resolved, "Carlito");
            assert_eq!(
                r.substitutions.get("calibri"),
                Some(&Some("Carlito".to_string()))
            );
        } else {
            assert_eq!(resolved, "calibri");
            assert_eq!(r.substitutions.get("calibri"), Some(&None));
        }
    }

    // Regression guard: the embedded metric-compatible faces must be available on
    // every platform (not gated to Android), so headless/CI/PDF-export builds can
    // register them. Re-gating to `target_os = "android"` would fail this here.
    #[test]
    fn fallback_font_blobs_embedded_on_all_targets() {
        assert!(
            !loki_fonts::fallback_font_blobs().is_empty(),
            "metric-compatible fallback faces must be embedded on this target"
        );
    }

    // Regression guard for the actual rendering bug: resolving a font with a known
    // metric-compatible substitute must yield a family that is *actually present*
    // in the collection. Before lazy fallback registration, "Calibri" resolved to
    // itself but Carlito was absent on desktop → Parley fell back to a digit-less
    // font, so list markers and Calibri text rendered `.notdef`.
    #[test]
    fn substituted_family_is_actually_available() {
        let mut r = FontResources::new();
        for requested in ["Calibri", "Arial", "Times New Roman", "Cambria"] {
            let resolved = r.resolve_font_name(requested);
            assert!(
                r.font_cx.collection.family_id(resolved.as_str()).is_some(),
                "{requested:?} resolved to {resolved:?}, which is not available in the collection",
            );
        }
    }
}
