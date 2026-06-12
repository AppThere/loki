// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List attributes for ordered lists.

/// The number style for an ordered list.
///
/// Corresponds to pandoc `ListNumberStyle`. Used by [`ListAttributes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ListNumberStyle {
    /// Use the default number style for the context.
    #[default]
    DefaultStyle,
    /// An example list (used for code examples in pandoc).
    Example,
    /// Arabic numerals: 1, 2, 3.
    Decimal,
    /// Lowercase Roman numerals: i, ii, iii.
    LowerRoman,
    /// Uppercase Roman numerals: I, II, III.
    UpperRoman,
    /// Lowercase Latin letters: a, b, c.
    LowerAlpha,
    /// Uppercase Latin letters: A, B, C.
    UpperAlpha,
}

/// The delimiter style around ordered list numbers.
///
/// Corresponds to pandoc `ListNumberDelim`. Used by [`ListAttributes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ListDelimiter {
    /// Use the default delimiter for the context.
    #[default]
    DefaultDelim,
    /// A period after the number: `1.`
    Period,
    /// A closing parenthesis: `1)`
    OneParen,
    /// Parentheses around the number: `(1)`
    TwoParens,
}

/// Attributes for an ordered list.
///
/// Corresponds to pandoc `ListAttributes = (Int, ListNumberStyle, ListNumberDelim)`.
/// Used by [`super::Block::OrderedList`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListAttributes {
    /// The starting number for the list.
    pub start_number: i32,
    /// The numbering style.
    pub style: ListNumberStyle,
    /// The delimiter style.
    pub delimiter: ListDelimiter,
}

impl Default for ListAttributes {
    fn default() -> Self {
        Self {
            start_number: 1,
            style: ListNumberStyle::Decimal,
            delimiter: ListDelimiter::Period,
        }
    }
}
