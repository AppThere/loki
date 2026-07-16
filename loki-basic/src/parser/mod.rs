// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The recursive-descent parser: [`Token`] stream → [`ast::Module`].
//!
//! Split across submodules by grammar area — expressions ([`expr`]),
//! statements ([`stmt`]), and declarations ([`decl`]) — over a shared token
//! cursor. The parser owns all keyword knowledge (the lexer is
//! keyword-agnostic); keyword matches are case-insensitive.

mod decl;
mod expr;
mod stmt;
mod stmt_block;

use crate::ast::{Item, Module, ModuleOptions};
use crate::dialect::Dialect;
use crate::error::BasicError;
use crate::lexer::{Lexer, Token, TokenKind};

/// A recursive-descent BASIC parser over a token buffer.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    dialect: Dialect,
    options: ModuleOptions,
}

impl Parser {
    /// Lexes and parses `src` into a [`Module`] for the given dialect.
    ///
    /// # Errors
    ///
    /// Returns [`BasicError::Lex`] or [`BasicError::Parse`] on malformed input.
    pub fn parse_module(src: &str, dialect: Dialect) -> Result<Module, BasicError> {
        let tokens = Lexer::new(src).tokenize()?;
        let mut p = Parser {
            tokens,
            pos: 0,
            dialect,
            options: ModuleOptions {
                base: dialect.default_option_base(),
                explicit: false,
                compare_text: false,
            },
        };
        p.parse_module_body()
    }

    fn parse_module_body(&mut self) -> Result<Module, BasicError> {
        let mut items = Vec::new();
        let mut name = None;
        self.skip_terminators();
        while !self.at_eof() {
            // `Option …`, `Attribute …`, and blank lines are handled here; every
            // other leading token begins a top-level item.
            if self.eat_kw("Option") {
                self.parse_option()?;
            } else if self.peek().is_kw("Attribute") {
                if let Some(n) = self.parse_attribute()? {
                    name = name.or(Some(n));
                }
            } else if let Some(item) = self.parse_item()? {
                items.push(item);
            }
            self.skip_terminators();
        }
        Ok(Module {
            name,
            dialect: self.dialect,
            options: self.options,
            items,
        })
    }

    /// Parses one top-level item, or `None` for a line that produced no item.
    fn parse_item(&mut self) -> Result<Option<Item>, BasicError> {
        decl::parse_item(self)
    }

    fn parse_option(&mut self) -> Result<(), BasicError> {
        if self.eat_kw("Base") {
            let n = self.expect_int_literal()?;
            self.options.base = i32::try_from(n).unwrap_or(0);
        } else if self.eat_kw("Explicit") {
            self.options.explicit = true;
        } else if self.eat_kw("Compare") {
            self.options.compare_text = self.eat_kw("Text");
            if !self.options.compare_text {
                // `Binary` (or `Database`, treated as binary) — consume the word.
                let _ = self.eat_ident();
            }
        } else {
            return Err(self.error("unknown Option directive"));
        }
        self.end_of_statement()
    }

    /// `Attribute VB_Name = "Module1"` → module name; other attributes ignored.
    fn parse_attribute(&mut self) -> Result<Option<String>, BasicError> {
        self.bump(); // "Attribute"
        let key = self.eat_ident().unwrap_or_default();
        let mut name = None;
        if self.eat(&TokenKind::Eq)
            && let TokenKind::Str(s) = &self.peek().kind
        {
            if key.eq_ignore_ascii_case("VB_Name") {
                name = Some(s.clone());
            }
            self.bump();
        }
        self.end_of_statement()?;
        Ok(name)
    }

    // ── Cursor primitives (shared by submodules) ────────────────────────────

    pub(super) fn peek(&self) -> &Token {
        // The token buffer always ends with Eof, so indexing the last token is
        // safe once `pos` reaches the end.
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("token buffer always contains Eof")
        })
    }

    pub(super) fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    /// The kind of the token `n` positions ahead of the cursor.
    pub(super) fn peek_at(&self, n: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + n)
            .map_or(&TokenKind::Eof, |t| &t.kind)
    }

    /// The token after the current one.
    pub(super) fn peek2_kind(&self) -> &TokenKind {
        self.peek_at(1)
    }

    pub(super) fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    pub(super) fn at_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    pub(super) fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(super) fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<(), BasicError> {
        if self.eat(kind) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {what}")))
        }
    }

    /// Consumes the current token if it is an identifier, returning its text.
    pub(super) fn eat_ident(&mut self) -> Option<String> {
        if let TokenKind::Ident(s) = self.peek_kind() {
            let s = s.clone();
            self.bump();
            Some(s)
        } else {
            None
        }
    }

    pub(super) fn expect_ident(&mut self, what: &str) -> Result<String, BasicError> {
        self.eat_ident()
            .ok_or_else(|| self.error(&format!("expected {what}")))
    }

    /// Case-insensitive keyword match on the current token (no consume).
    pub(super) fn peek_kw(&self, kw: &str) -> bool {
        self.peek().is_kw(kw)
    }

    /// Consumes the current token if it is the given keyword.
    pub(super) fn eat_kw(&mut self, kw: &str) -> bool {
        if self.peek_kw(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_int_literal(&mut self) -> Result<i64, BasicError> {
        if let TokenKind::Int(n) = *self.peek_kind() {
            self.bump();
            Ok(n)
        } else {
            Err(self.error("expected an integer literal"))
        }
    }

    // ── Statement termination ───────────────────────────────────────────────

    /// A statement ends at a newline, a `:` separator, or EOF.
    pub(super) fn at_stmt_end(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Newline | TokenKind::Colon | TokenKind::Eof
        )
    }

    /// Consumes one statement terminator, erroring if the statement did not end
    /// where expected.
    pub(super) fn end_of_statement(&mut self) -> Result<(), BasicError> {
        if self.at_eof() {
            return Ok(());
        }
        if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Colon) {
            self.bump();
            Ok(())
        } else {
            Err(self.error("expected end of statement"))
        }
    }

    /// Skips any run of blank statement terminators.
    pub(super) fn skip_terminators(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Colon) {
            self.bump();
        }
    }

    pub(super) fn error(&self, message: &str) -> BasicError {
        BasicError::Parse {
            message: message.to_string(),
            span: self.peek().span,
        }
    }
}
