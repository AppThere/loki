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

//! Tab stop definitions for paragraph formatting.
//!
//! TR 29166 §6.2.2 "Paragraph formatting" includes tab stop lists.
//! ODF: `style:tab-stop` inside `style:paragraph-properties`.
//! OOXML: `w:tab` inside `w:tabs` inside `w:pPr`.

use loki_primitives::units::Points;

/// The alignment of text at a tab stop position.
///
/// TR 29166 §6.2.2. ODF `style:type`; OOXML `w:val` on `w:tab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TabAlignment {
    /// Text is left-aligned at the tab stop (the default).
    #[default]
    Left,
    /// Text is right-aligned at the tab stop.
    Right,
    /// Text is centered at the tab stop.
    Center,
    /// Text is aligned on the decimal separator character.
    Decimal,
    /// A tab stop used to clear an inherited tab stop (OOXML `clear`).
    Clear,
}

/// The leader character displayed between the end of text and the tab stop.
///
/// ODF `style:leader-type`; OOXML `w:leader` on `w:tab`.
/// TR 29166 §6.2.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TabLeader {
    /// No leader character (the default).
    #[default]
    None,
    /// A dotted leader (`...`).
    Dot,
    /// A dashed leader (`---`).
    Dash,
    /// An underscore leader (`___`).
    Underscore,
    /// A heavy solid leader.
    Heavy,
    /// A middle-dot leader.
    MiddleDot,
}

/// A single tab stop definition.
///
/// Describes a custom tab stop within a paragraph's tab stop list.
/// TR 29166 §6.2.2 "Tab stops".
///
/// ODF: `style:tab-stop` with `style:position`, `style:type`, and
/// `style:leader-*` attributes.
/// OOXML: `w:tab` with `w:pos`, `w:val`, and `w:leader` attributes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabStop {
    /// The position of the tab stop from the start of the line, in points.
    pub position: Points,
    /// The text alignment at this tab stop.
    pub alignment: TabAlignment,
    /// The leader character shown between text and the tab stop.
    pub leader: TabLeader,
}

impl TabStop {
    /// Creates a simple left-aligned tab stop at the given position with
    /// no leader.
    #[must_use]
    pub fn left(position: Points) -> Self {
        Self {
            position,
            alignment: TabAlignment::Left,
            leader: TabLeader::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_alignment_is_left() {
        assert_eq!(TabAlignment::default(), TabAlignment::Left);
    }

    #[test]
    fn tab_stop_left_constructor() {
        let ts = TabStop::left(Points::new(36.0));
        assert_eq!(ts.alignment, TabAlignment::Left);
        assert_eq!(ts.leader, TabLeader::None);
    }
}
