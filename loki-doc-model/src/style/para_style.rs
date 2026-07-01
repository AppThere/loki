// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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

    /// The style to apply to the paragraph created when the user presses Enter
    /// at the end of a paragraph with this style. `None` means the same style
    /// continues. ODF: `style:next-style-name`; OOXML: `w:next @w:val`.
    ///
    // TODO(editing): next_style_id used by split_block to determine
    // the style of the newly created paragraph after Enter.
    pub next_style_id: Option<String>,

    /// Whether this is the document's default paragraph style.
    /// At most one style in the catalog may have `is_default = true`.
    pub is_default: bool,

    /// Whether this is a custom (user-defined) style as opposed to a
    /// built-in style from the application.
    pub is_custom: bool,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

impl ParagraphStyle {
    /// Whether this is a **built-in** (application-provided) style rather than a
    /// user-created one — the styles the management panel protects from deletion
    /// and rename (Spec 05 §8 / audit SM-11).
    ///
    /// The rule is the existing model flags: a style is built-in when it is the
    /// document default (`is_default`) or is not user-custom (`!is_custom`). Spec
    /// 05 assumed `COMPAT(i18n)` annotations on internal match keys would mark
    /// built-ins; those do not exist and are not needed — `is_custom` already
    /// carries the built-in-vs-user distinction, so that framing is dropped.
    #[must_use]
    pub fn is_builtin(&self) -> bool {
        self.is_default || !self.is_custom
    }
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
            next_style_id: None,
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

    fn style(is_default: bool, is_custom: bool) -> ParagraphStyle {
        ParagraphStyle {
            id: StyleId("S".into()),
            display_name: None,
            parent: None,
            linked_char_style: None,
            next_style_id: None,
            para_props: ParaProps::default(),
            char_props: CharProps::default(),
            is_default,
            is_custom,
            extensions: ExtensionBag::default(),
        }
    }

    #[test]
    fn is_builtin_distinguishes_application_styles_from_user_styles() {
        // Built-in: not user-custom.
        assert!(style(false, false).is_builtin());
        // The document default is always built-in (protected), even if flagged custom.
        assert!(style(true, true).is_builtin());
        // A user-created custom style is not built-in.
        assert!(!style(false, true).is_builtin());
    }
}
