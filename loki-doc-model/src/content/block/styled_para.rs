// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Styled paragraph type for office documents.

use crate::content::attr::NodeAttr;
use crate::content::inline::Inline;
use crate::style::catalog::StyleId;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;

/// A styled paragraph: paragraph content plus a style reference and optional
/// direct formatting overrides.
///
/// This is the primary paragraph type for office documents.
/// TR 29166 §7.2.2 (paragraph structure) and §7.2.3 (styles).
///
/// ODF: `text:p` with `text:style-name`. OOXML: `w:p` with `w:pStyle`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyledParagraph {
    /// Reference to a named paragraph style in the style catalog.
    /// `None` = no named style; uses document defaults.
    pub style_id: Option<StyleId>,
    /// Direct paragraph formatting overrides.
    /// `None` = no direct formatting.
    pub direct_para_props: Option<Box<ParaProps>>,
    /// Direct character formatting overrides applied to the paragraph mark.
    pub direct_char_props: Option<Box<CharProps>>,
    /// The inline content of the paragraph.
    pub inlines: Vec<Inline>,
    /// Generic node attributes.
    pub attr: NodeAttr,
}
