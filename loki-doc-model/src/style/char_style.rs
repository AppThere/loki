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

//! Named character (run-level) style definition.
//!
//! A character style applies character-level formatting to a run of inline
//! content. TR 29166 §7.2.3 compares the ODF and OOXML character style models.

use crate::content::attr::ExtensionBag;
use crate::style::catalog::StyleId;
use crate::style::props::char_props::CharProps;

/// A named character style (run-level formatting).
///
/// Applied via [`crate::content::inline::Inline::StyledRun`].
/// TR 29166 §7.2.3 (Styles XML structure comparison).
///
/// ODF: `style:style style:family="text"`.
/// OOXML: `w:style w:type="character"`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharacterStyle {
    /// The unique identifier used to reference this style.
    pub id: StyleId,

    /// A human-readable display name shown in the UI.
    /// ODF `style:display-name`; OOXML `w:name`.
    pub display_name: Option<String>,

    /// The parent style identifier. `None` means this style has no parent
    /// and inherits only from the document default character properties.
    /// ODF `style:parent-style-name`; OOXML `w:basedOn`.
    pub parent: Option<StyleId>,

    /// The character-level formatting properties defined by this style.
    /// `None` fields are inherited from `parent`.
    pub char_props: CharProps,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_style_no_parent() {
        let style = CharacterStyle {
            id: StyleId("emphasis".into()),
            display_name: Some("Emphasis".into()),
            parent: None,
            char_props: CharProps {
                italic: Some(true),
                ..Default::default()
            },
            extensions: ExtensionBag::default(),
        };
        assert!(style.parent.is_none());
        assert_eq!(style.char_props.italic, Some(true));
    }
}
