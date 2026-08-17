// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Hard line breaks across the CRDT boundary.
//!
//! The write path stores [`Inline::LineBreak`] as a literal `U+000A` in the
//! paragraph's Loro text (`inlines.rs`), because the break has to occupy a
//! caret position — the editor addresses inline content by byte offset, so a
//! break that took no bytes could not be selected, stepped over, or deleted.
//!
//! Nothing reconstructed it on the way back, so a break survived only until the
//! first re-derive: it returned inside an [`Inline::Str`], and DOCX export
//! writes a `Str` verbatim into `<w:t>`, where a newline is whitespace rather
//! than a break. Every edit re-derives the document from Loro, so in practice a
//! line break was lost as soon as the user typed anything — it exported as
//! `<w:t>` whitespace instead of `<w:br/>`.
//!
//! [`text_to_inlines`] closes that loop: `\n` in CRDT text reads back as
//! [`Inline::LineBreak`], which the DOCX and ODT writers already handle.

use crate::content::inline::Inline;

/// Splits one CRDT text span into `Str` runs separated by
/// [`Inline::LineBreak`].
///
/// The inverse of the write path's `LineBreak => text.insert("\n")`. Empty runs
/// between adjacent breaks are dropped — two consecutive breaks are two
/// `LineBreak`s and no empty `Str` — but the byte count is unchanged either
/// way, because a `LineBreak` contributes the same single `\n` to the text
/// extraction that feeds cursor offsets (`inlines.rs`'s `push('\n')`).
///
/// Text with no newline — overwhelmingly the common case — yields exactly one
/// `Str`, allocating no more than the previous code did.
#[must_use]
pub(super) fn text_to_inlines(text: &str) -> Vec<Inline> {
    if !text.contains('\n') {
        return vec![Inline::Str(text.to_owned())];
    }
    let mut out = Vec::new();
    for (i, piece) in text.split('\n').enumerate() {
        if i > 0 {
            out.push(Inline::LineBreak);
        }
        if !piece.is_empty() {
            out.push(Inline::Str(piece.to_owned()));
        }
    }
    out
}

#[cfg(test)]
#[path = "line_breaks_tests.rs"]
mod tests;
