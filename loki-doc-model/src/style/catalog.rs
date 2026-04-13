// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! The document style catalog.
//!
//! [`StyleCatalog`] is the registry of all named styles in a document.
//! Both ODF (`<office:styles>`, `<office:automatic-styles>`) and OOXML
//! (`word/styles.xml`) maintain such a catalog. TR 29166 §7.2.3.
//!
//! [`indexmap::IndexMap`] is used instead of `HashMap` to
//! preserve insertion order — important for reproducible serialization.
//! See ADR-0007.

use indexmap::IndexMap;
use crate::style::char_style::CharacterStyle;
use crate::style::list_style::ListStyle;
use crate::style::para_style::ParagraphStyle;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;
use crate::style::table_style::TableStyle;

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
}

impl StyleCatalog {
    /// Creates an empty [`StyleCatalog`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves the paragraph properties for a style by walking the parent
    /// chain and merging properties (child wins over parent). ADR-0003.
    ///
    /// Returns `None` if the style id is not in the catalog.
    #[must_use]
    pub fn resolve_para(&self, id: &StyleId) -> Option<ResolvedParaProps> {
        let style = self.paragraph_styles.get(id)?;
        let own = style.para_props.clone();
        if let Some(ref parent_id) = style.parent
            && let Some(parent_resolved) = self.resolve_para(parent_id)
        {
            return Some(own.merged_with_parent(&parent_resolved));
        }
        Some(own)
    }

    /// Resolves the character properties for a paragraph style by walking
    /// the parent chain. ADR-0003.
    ///
    /// Returns `None` if the style id is not in the catalog.
    #[must_use]
    pub fn resolve_char(&self, id: &StyleId) -> Option<ResolvedCharProps> {
        let style = self.paragraph_styles.get(id)?;
        let own = style.char_props.clone();
        if let Some(ref parent_id) = style.parent
            && let Some(parent_resolved) = self.resolve_char(parent_id)
        {
            return Some(own.merged_with_parent(&parent_resolved));
        }
        Some(own)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::attr::ExtensionBag;
    use loki_primitives::units::Points;

    fn make_catalog_with_parent_child() -> StyleCatalog {
        let mut catalog = StyleCatalog::new();

        let parent = ParagraphStyle {
            id: StyleId::new("Normal"),
            display_name: Some("Normal".into()),
            parent: None,
            linked_char_style: None,
            para_props: ParaProps::default(),
            char_props: CharProps {
                font_size: Some(Points::new(12.0)),
                bold: Some(false),
                ..Default::default()
            },
            is_default: true,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };

        let child = ParagraphStyle {
            id: StyleId::new("Heading1"),
            display_name: Some("Heading 1".into()),
            parent: Some(StyleId::new("Normal")),
            linked_char_style: None,
            para_props: ParaProps::default(),
            char_props: CharProps {
                font_size: Some(Points::new(24.0)),
                bold: Some(true),
                ..Default::default()
            },
            is_default: false,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };

        catalog.paragraph_styles.insert(StyleId::new("Normal"), parent);
        catalog.paragraph_styles.insert(StyleId::new("Heading1"), child);
        catalog
    }

    #[test]
    fn resolve_child_overrides_parent() {
        let catalog = make_catalog_with_parent_child();
        let resolved = catalog.resolve_char(&StyleId::new("Heading1")).unwrap();
        assert_eq!(resolved.font_size, Some(Points::new(24.0)));
        assert_eq!(resolved.bold, Some(true));
    }

    #[test]
    fn resolve_child_inherits_parent_unset() {
        let catalog = make_catalog_with_parent_child();
        // The parent has font_size=12pt. The child overrides to 24pt.
        // But italic is None in both — should still be None after resolution.
        let resolved = catalog.resolve_char(&StyleId::new("Heading1")).unwrap();
        assert!(resolved.italic.is_none());
    }

    #[test]
    fn resolve_missing_style_returns_none() {
        let catalog = StyleCatalog::new();
        assert!(catalog.resolve_para(&StyleId::new("NonExistent")).is_none());
    }
}
