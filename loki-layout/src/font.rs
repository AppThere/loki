// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared Parley font and layout context.

use std::collections::HashMap;
use std::sync::Arc;

use crate::color::LayoutColor;

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
}

impl FontResources {
    /// Creates a new `FontResources`, loading system fonts via Fontique.
    pub fn new() -> Self {
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

        Self {
            font_cx,
            layout_cx: parley::LayoutContext::new(),
            font_data_cache: HashMap::new(),
            substitutions: HashMap::new(),
        }
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
            // Check if the substitute is available.
            if self.font_cx.collection.family_id(sub_name).is_some() {
                self.substitutions
                    .insert(name.to_string(), Some(sub_name.to_string()));
                return sub_name.to_string();
            }
        }

        // No named substitute found — try common system/bundled fonts as generic fallbacks.
        const GENERIC_FALLBACKS: &[&str] =
            &["Segoe UI", "Noto Sans", "Liberation Sans", "DejaVu Sans"];
        for fallback in GENERIC_FALLBACKS {
            if self.font_cx.collection.family_id(fallback).is_some() {
                self.substitutions
                    .insert(name.to_string(), Some(fallback.to_string()));
                return fallback.to_string();
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
}
