// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Named paragraph style definition.
//!
//! A paragraph style defines both paragraph-level and character-level
//! properties. TR 29166 §7.2.3 describes the ODF/OOXML style model
//! comparison.

use crate::content::attr::ExtensionBag;
use crate::style::catalog::StyleId;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;

/// A named paragraph style.
///
/// Applied via [`crate::content::block::StyledParagraph`].
/// TR 29166 §7.2.3 (Styles XML structure comparison).
///
/// ODF: `style:style style:family="paragraph"`.
/// OOXML: `w:style w:type="paragraph"`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParagraphStyle {
    /// The unique identifier used to reference this style.
    pub id: StyleId,

    /// A human-readable display name shown in the UI.
    /// ODF `style:display-name`; OOXML `w:name`.
    pub display_name: Option<String>,

    /// The parent style identifier. `None` means this style is the root
    /// (inherits only from document defaults).
    /// ODF `style:parent-style-name`; OOXML `w:basedOn`.
    pub parent: Option<StyleId>,

    /// An optional linked character style that applies character formatting
    /// to the paragraph mark. ODF `style:linked-style-name`;
    /// OOXML `w:link`.
    pub linked_char_style: Option<StyleId>,

    /// Paragraph-level formatting properties.
    pub para_props: ParaProps,

    /// Character-level formatting properties applied to the entire paragraph
    /// (runs without an explicit character style inherit these).
    pub char_props: CharProps,

    /// Whether this is the document's default paragraph style.
    /// At most one style in the catalog may have `is_default = true`.
    pub is_default: bool,

    /// Whether this is a custom (user-defined) style as opposed to a
    /// built-in style from the application.
    pub is_custom: bool,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_style_with_parent() {
        let parent_id = StyleId("Normal".into());
        let style = ParagraphStyle {
            id: StyleId("Heading1".into()),
            display_name: Some("Heading 1".into()),
            parent: Some(parent_id.clone()),
            linked_char_style: None,
            para_props: ParaProps::default(),
            char_props: CharProps {
                bold: Some(true),
                font_size: Some(loki_primitives::units::Points::new(24.0)),
                ..Default::default()
            },
            is_default: false,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };
        assert_eq!(style.parent, Some(parent_id));
        assert_eq!(style.char_props.bold, Some(true));
    }
}
