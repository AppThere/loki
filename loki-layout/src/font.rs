// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared Parley font and layout context.
//!
//! [`FontResources`] wraps a [`parley::FontContext`] (font database) and a
//! [`parley::LayoutContext`] (shaping scratch space). Both are expensive to
//! construct and should be reused across many layout calls.

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
}


impl FontResources {
    /// Creates a new `FontResources`, loading system fonts via Fontique.
    pub fn new() -> Self {
        Self {
            font_cx: parley::FontContext::new(),
            layout_cx: parley::LayoutContext::new(),
            font_data_cache: HashMap::new(),
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
}

impl Default for FontResources {
    fn default() -> Self {
        Self::new()
    }
}
