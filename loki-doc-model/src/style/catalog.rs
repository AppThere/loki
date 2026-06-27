// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The document style catalog.
//!
//! [`StyleCatalog`] is the registry of all named styles in a document.
//! Both ODF (`<office:styles>`, `<office:automatic-styles>`) and OOXML
//! (`word/styles.xml`) maintain such a catalog. TR 29166 §7.2.3.
//!
//! [`indexmap::IndexMap`] is used instead of `HashMap` to
//! preserve insertion order — important for reproducible serialization.
//! See ADR-0007.

use crate::style::char_style::CharacterStyle;
use crate::style::list_style::ListStyle;
use crate::style::para_style::ParagraphStyle;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;
use crate::style::table_style::TableStyle;
use indexmap::IndexMap;

/// Maximum number of parent links followed when resolving a style chain.
///
/// Guards against cyclic `parent` references in corrupt documents
/// (e.g. A.parent = B, B.parent = A), which would otherwise loop forever.
/// When the cap is exceeded, inheritance stops at the last style reached —
/// the chain is treated as if it ended at a root style.
pub const MAX_STYLE_CHAIN_DEPTH: usize = 32;

/// Unique identifier for a named style.
///
/// Used to reference a style from content nodes and from other styles
/// (via `parent` fields). TR 29166 §7.2.3.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyleId(pub String);

impl StyleId {
    /// Creates a new [`StyleId`] from the given string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the style id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StyleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Fully resolved character properties after walking the parent chain.
///
/// All fields are concrete values (never `None` for font-critical properties
/// once the chain is fully resolved). Used by renderers and exporters.
pub type ResolvedCharProps = CharProps;

/// Fully resolved paragraph properties after walking the parent chain.
pub type ResolvedParaProps = ParaProps;

/// The document's named style catalog.
///
/// Both ODF (via `<office:styles>`, `<office:automatic-styles>`) and OOXML
/// (via `word/styles.xml`) maintain a catalog of named styles. This type
/// provides a format-neutral representation.
///
/// `IndexMap` is used to preserve insertion order for reproducible
/// serialization. See ADR-0007.
///
/// TR 29166 §7.2.3 (Styles XML structure comparison).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyleCatalog {
    /// Named paragraph styles. ODF `style:family="paragraph"`;
    /// OOXML `w:type="paragraph"`.
    pub paragraph_styles: IndexMap<StyleId, ParagraphStyle>,
    /// Named character styles. ODF `style:family="text"`;
    /// OOXML `w:type="character"`.
    pub character_styles: IndexMap<StyleId, CharacterStyle>,
    /// Named table styles. ODF `style:family="table"`;
    /// OOXML `w:type="table"`.
    pub table_styles: IndexMap<StyleId, TableStyle>,
    /// Named list styles. ODF `text:list-style`;
    /// OOXML `w:abstractNum`.
    pub list_styles: IndexMap<crate::style::list_style::ListId, ListStyle>,
    /// The id of the document's **default paragraph style** — the style a
    /// paragraph with no explicit style reference inherits from. OOXML: the
    /// paragraph style with `w:default="1"` (typically `Normal`, rooted at
    /// `w:docDefaults`); ODF: the `style:default-style` for paragraphs. `None`
    /// means "no document default" (a bare paragraph resolves to engine defaults).
    ///
    /// Without this, default-font body text (no `w:pStyle`) would bypass the
    /// `docDefaults` chain and lose the document's base font, causing wrong-font
    /// rendering and pagination drift.
    #[cfg_attr(feature = "serde", serde(default))]
    pub default_paragraph_style: Option<StyleId>,
}

impl StyleCatalog {
    /// Creates an empty [`StyleCatalog`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the style id to resolve for a paragraph, given its (possibly
    /// absent) explicit style reference: the explicit id if present, otherwise
    /// the document's [`default_paragraph_style`](Self::default_paragraph_style).
    /// Mirrors OOXML/ODF semantics where a paragraph with no style still
    /// inherits the default paragraph style (and through it, `docDefaults`).
    #[must_use]
    pub fn effective_paragraph_style<'a>(
        &'a self,
        explicit: Option<&'a StyleId>,
    ) -> Option<&'a StyleId> {
        explicit.or(self.default_paragraph_style.as_ref())
    }

    /// Resolves the paragraph properties for a style by walking the parent
    /// chain and merging properties (child wins over parent). ADR-0003.
    ///
    /// The walk is capped at [`MAX_STYLE_CHAIN_DEPTH`] links so that cyclic
    /// `parent` references in corrupt documents degrade gracefully (the chain
    /// is truncated) instead of overflowing the stack.
    ///
    /// Returns `None` if the style id is not in the catalog.
    #[must_use]
    pub fn resolve_para(&self, id: &StyleId) -> Option<ResolvedParaProps> {
        let style = self.paragraph_styles.get(id)?;
        let mut resolved = style.para_props.clone();
        let mut parent_id = style.parent.as_ref();
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let Some(parent) = parent_id.and_then(|pid| self.paragraph_styles.get(pid)) else {
                break;
            };
            resolved = resolved.merged_with_parent(&parent.para_props);
            parent_id = parent.parent.as_ref();
        }
        Some(resolved)
    }

    /// Resolves the character properties for a paragraph style by walking
    /// the parent chain. ADR-0003.
    ///
    /// The walk is capped at [`MAX_STYLE_CHAIN_DEPTH`] links so that cyclic
    /// `parent` references in corrupt documents degrade gracefully (the chain
    /// is truncated) instead of overflowing the stack.
    ///
    /// Returns `None` if the style id is not in the catalog.
    #[must_use]
    pub fn resolve_char(&self, id: &StyleId) -> Option<ResolvedCharProps> {
        let style = self.paragraph_styles.get(id)?;
        let mut resolved = style.char_props.clone();
        let mut parent_id = style.parent.as_ref();
        for _ in 0..MAX_STYLE_CHAIN_DEPTH {
            let Some(parent) = parent_id.and_then(|pid| self.paragraph_styles.get(pid)) else {
                break;
            };
            resolved = resolved.merged_with_parent(&parent.char_props);
            parent_id = parent.parent.as_ref();
        }
        Some(resolved)
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
