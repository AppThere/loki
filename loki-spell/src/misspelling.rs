// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`Misspelling`] result type.

use core::ops::Range;

/// A misspelled word located within a run of checked text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misspelling {
    /// The misspelled word (owned, for use in a suggestions menu).
    pub word: String,
    /// Byte range of the word within the text passed to `check_text`.
    pub range: Range<usize>,
}
