// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The BASIC lexer: source text → [`Token`] stream.
//!
//! Line-oriented and case-insensitive. Handles statement separators (newline
//! and `:`), ` _` line continuations, `'` comments, string literals with `""`
//! escaping, `#…#` date literals, `&H`/`&O` radix integers, and type-suffix
//! characters (`% ! # @ $ &`, consumed and discarded — the Variant value model
//! does not need them; declared types use the `As` form).

mod number;
mod scan;
mod token;

pub use token::{Token, TokenKind};

use crate::error::{BasicError, Span};

/// A streaming lexer over a source string.
pub struct Lexer<'a> {
    #[allow(dead_code)]
    src: &'a str,
    chars: Vec<(usize, char)>,
    i: usize,
    /// Byte length of the source, used as the EOF span position.
    len: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer over `src`.
    #[must_use]
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            chars: src.char_indices().collect(),
            i: 0,
            len: src.len(),
        }
    }

    /// Tokenises the entire input, returning all tokens ending with
    /// [`TokenKind::Eof`].
    ///
    /// # Errors
    ///
    /// Returns [`BasicError::Lex`] on an unterminated string/date literal or a
    /// malformed numeric literal.
    pub fn tokenize(mut self) -> Result<Vec<Token>, BasicError> {
        let mut out = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            out.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(out)
    }

    // ── Cursor helpers (pub(super) for the number submodule) ────────────────

    pub(super) fn peek(&self) -> Option<char> {
        self.chars.get(self.i).map(|&(_, c)| c)
    }

    pub(super) fn peek2(&self) -> Option<char> {
        self.chars.get(self.i + 1).map(|&(_, c)| c)
    }

    pub(super) fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    /// Byte offset of the current position (or end-of-source at EOF).
    pub(super) fn offset(&self) -> usize {
        self.chars.get(self.i).map_or(self.len, |&(o, _)| o)
    }

    /// The current character-index cursor (for continuation lookahead).
    pub(super) fn cursor(&self) -> usize {
        self.i
    }

    /// The character at character-index `j`, if any.
    pub(super) fn char_at(&self, j: usize) -> Option<char> {
        self.chars.get(j).map(|&(_, c)| c)
    }

    /// Consumes one trailing type-suffix char (`% ! # @ $`, or `&` when it is a
    /// `Long` suffix rather than concat/`&H`) and discards it.
    pub(super) fn consume_type_suffix(&mut self) {
        match self.peek() {
            Some('%' | '!' | '#' | '@' | '$') => {
                self.bump();
            }
            Some('&') => {
                // `&` is a Long suffix only when it does not begin `&H`/`&O` and
                // is not followed by another operand char (which would make it
                // the concatenation operator).
                let next = self.peek2();
                let is_suffix = !matches!(next, Some(c) if c.is_ascii_alphanumeric());
                if is_suffix {
                    self.bump();
                }
            }
            _ => {}
        }
    }

    // ── Main scan ───────────────────────────────────────────────────────────

    fn next_token(&mut self) -> Result<Token, BasicError> {
        self.skip_inline_ws_and_continuations();
        let start = self.offset();
        let Some(c) = self.peek() else {
            return Ok(Token::new(TokenKind::Eof, Span::new(self.len, self.len)));
        };

        // Line breaks → a single Newline token (collapse CRLF).
        if c == '\n' || c == '\r' {
            self.bump();
            if c == '\r' && self.peek() == Some('\n') {
                self.bump();
            }
            return Ok(Token::new(
                TokenKind::Newline,
                Span::new(start, self.offset()),
            ));
        }

        // Comment to end of line.
        if c == '\'' {
            self.skip_to_line_end();
            return self.next_token();
        }

        if c == '"' {
            return self.scan_string(start);
        }
        if c == '#' {
            return self.scan_date(start);
        }
        if c.is_ascii_digit() || (c == '.' && self.peek2().is_some_and(|d| d.is_ascii_digit())) {
            let kind = number::scan_number(self, start)?;
            return Ok(Token::new(kind, Span::new(start, self.offset())));
        }
        if c == '&' && matches!(self.peek2(), Some('h' | 'H' | 'o' | 'O')) {
            let kind = number::scan_number(self, start)?;
            return Ok(Token::new(kind, Span::new(start, self.offset())));
        }
        if is_ident_start(c) {
            return Ok(self.scan_ident(start));
        }
        self.scan_operator(start)
    }

    fn scan_ident(&mut self, start: usize) -> Token {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        self.consume_type_suffix();
        Token::new(TokenKind::Ident(s), Span::new(start, self.offset()))
    }

    fn scan_string(&mut self, start: usize) -> Result<Token, BasicError> {
        self.bump(); // opening quote
        let mut s = String::new();
        loop {
            match self.bump() {
                Some('"') => {
                    if self.peek() == Some('"') {
                        self.bump();
                        s.push('"');
                    } else {
                        return Ok(Token::new(
                            TokenKind::Str(s),
                            Span::new(start, self.offset()),
                        ));
                    }
                }
                Some(c) => s.push(c),
                None => {
                    return Err(BasicError::Lex {
                        message: "unterminated string literal".into(),
                        span: Span::new(start, self.offset()),
                    });
                }
            }
        }
    }

    fn scan_date(&mut self, start: usize) -> Result<Token, BasicError> {
        self.bump(); // opening '#'
        let mut s = String::new();
        loop {
            match self.bump() {
                Some('#') => {
                    return Ok(Token::new(
                        TokenKind::Date(s),
                        Span::new(start, self.offset()),
                    ));
                }
                Some('\n' | '\r') | None => {
                    return Err(BasicError::Lex {
                        message: "unterminated date literal".into(),
                        span: Span::new(start, self.offset()),
                    });
                }
                Some(c) => s.push(c),
            }
        }
    }

    fn scan_operator(&mut self, start: usize) -> Result<Token, BasicError> {
        let c = self.bump().unwrap_or('\0');
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '\\' => TokenKind::Backslash,
            '^' => TokenKind::Caret,
            '&' => TokenKind::Amp,
            '=' => TokenKind::Eq,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,
            '<' => match self.peek() {
                Some('=') => {
                    self.bump();
                    TokenKind::Le
                }
                Some('>') => {
                    self.bump();
                    TokenKind::Ne
                }
                _ => TokenKind::Lt,
            },
            '>' => {
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            other => {
                return Err(BasicError::Lex {
                    message: format!("unexpected character {other:?}"),
                    span: Span::new(start, self.offset()),
                });
            }
        };
        Ok(Token::new(kind, Span::new(start, self.offset())))
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod tests;
