// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Token types produced by the [`super::Lexer`].
//!
//! The lexer is intentionally *keyword-agnostic*: every alphabetic word becomes
//! an [`TokenKind::Ident`] and the parser owns all keyword knowledge (BASIC has
//! ~100 keywords, many context-sensitive, and several — `And`, `Mod`, `Is` — are
//! operators). This keeps the token set small and the keyword table in one
//! place. Identifier comparison is case-insensitive (see [`Token::is_kw`]).

use crate::error::Span;

/// A lexical token with its source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What kind of token this is.
    pub kind: TokenKind,
    /// Byte range in the source.
    pub span: Span,
}

impl Token {
    /// Creates a token.
    #[must_use]
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Returns the identifier text if this is an [`TokenKind::Ident`].
    #[must_use]
    pub fn ident(&self) -> Option<&str> {
        match &self.kind {
            TokenKind::Ident(s) => Some(s),
            _ => None,
        }
    }

    /// Case-insensitive keyword/identifier match (BASIC is case-insensitive).
    #[must_use]
    pub fn is_kw(&self, keyword: &str) -> bool {
        self.ident()
            .is_some_and(|s| s.eq_ignore_ascii_case(keyword))
    }
}

/// The kinds of token the lexer emits.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// An integer literal (decimal, `&H` hex, or `&O` octal).
    Int(i64),
    /// A floating-point literal.
    Float(f64),
    /// A string literal (with `""` already un-escaped to `"`).
    Str(String),
    /// A date literal — the raw text between the `#` delimiters (parsed later).
    Date(String),
    /// An identifier or keyword (original case preserved).
    Ident(String),

    // ── Arithmetic / string operators ───────────────────────────────────────
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `\` (integer division)
    Backslash,
    /// `^` (exponentiation)
    Caret,
    /// `&` (string concatenation)
    Amp,

    // ── Comparison / assignment ─────────────────────────────────────────────
    /// `=`
    Eq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `<>`
    Ne,

    // ── Punctuation ─────────────────────────────────────────────────────────
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `;`
    Semicolon,
    /// `:` (statement separator / label marker)
    Colon,

    // ── Structure ───────────────────────────────────────────────────────────
    /// End of a logical line (statement separator).
    Newline,
    /// End of input.
    Eof,
}
