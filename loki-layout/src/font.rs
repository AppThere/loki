// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Shared Parley font and layout context.
//!
//! [`FontResources`] wraps a [`parley::FontContext`] (font database) and a
//! [`parley::LayoutContext`] (shaping scratch space). Both are expensive to
//! construct and should be reused across many layout calls.

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
}

impl FontResources {
    /// Creates a new `FontResources`, loading system fonts via Fontique.
    pub fn new() -> Self {
        Self {
            font_cx: parley::FontContext::new(),
            layout_cx: parley::LayoutContext::new(),
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
