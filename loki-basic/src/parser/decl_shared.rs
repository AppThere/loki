// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared declaration-parsing building blocks used by both the top-level item
//! parser ([`super::decl`]) and the statement parser's local `Dim`/`Const`:
//! `As Type` clauses, variable/const declarations, array bounds, and the
//! `End <kw>` terminator.

use super::Parser;
use crate::ast::{ArrayBound, ConstDecl, TypeRef, VarDecl};
use crate::error::BasicError;
use crate::lexer::TokenKind;

// ── Shared building blocks (also used by statement-level Dim/Const) ─────────

impl Parser {
    /// Parses an optional `As Type` clause, returning [`TypeRef::Implicit`] when
    /// absent.
    pub(super) fn parse_as_type(&mut self) -> Result<TypeRef, BasicError> {
        if !self.eat_kw("As") {
            return Ok(TypeRef::Implicit);
        }
        let _ = self.eat_kw("New"); // `As New Class` — construction handled later.
        let name = self.expect_ident("a type name after `As`")?;
        if name.eq_ignore_ascii_case("String") && self.eat(&TokenKind::Star) {
            if let TokenKind::Int(n) = *self.peek_kind() {
                self.bump();
                let len = usize::try_from(n).unwrap_or(0);
                return Ok(TypeRef::FixedString(len));
            }
            return Err(self.error("expected a length after `String *`"));
        }
        Ok(TypeRef::Named(name))
    }

    /// Parses a comma-separated list of variable declarations (after `Dim`).
    pub(super) fn parse_var_decls(&mut self) -> Result<Vec<VarDecl>, BasicError> {
        let mut decls = vec![self.parse_one_var_decl()?];
        while self.eat(&TokenKind::Comma) {
            decls.push(self.parse_one_var_decl()?);
        }
        Ok(decls)
    }

    /// Parses one `name[(bounds)] [As Type]` variable declaration.
    pub(super) fn parse_one_var_decl(&mut self) -> Result<VarDecl, BasicError> {
        let name = self.expect_ident("a variable name")?;
        let bounds = if self.eat(&TokenKind::LParen) {
            if self.eat(&TokenKind::RParen) {
                Some(Vec::new()) // dynamic array `a()`
            } else {
                let b = self.parse_array_bounds()?;
                self.expect(&TokenKind::RParen, "`)` after array bounds")?;
                Some(b)
            }
        } else {
            None
        };
        let ty = self.parse_as_type()?;
        Ok(VarDecl { name, ty, bounds })
    }

    fn parse_array_bounds(&mut self) -> Result<Vec<ArrayBound>, BasicError> {
        let mut bounds = vec![self.parse_one_bound()?];
        while self.eat(&TokenKind::Comma) {
            bounds.push(self.parse_one_bound()?);
        }
        Ok(bounds)
    }

    fn parse_one_bound(&mut self) -> Result<ArrayBound, BasicError> {
        let first = self.parse_expr()?;
        if self.eat_kw("To") {
            let upper = self.parse_expr()?;
            Ok(ArrayBound {
                lower: Some(first),
                upper,
            })
        } else {
            Ok(ArrayBound {
                lower: None,
                upper: first,
            })
        }
    }

    /// Parses a comma-separated list of `name [As Type] = value` constants.
    pub(super) fn parse_const_decls(&mut self) -> Result<Vec<ConstDecl>, BasicError> {
        let mut decls = vec![self.parse_one_const()?];
        while self.eat(&TokenKind::Comma) {
            decls.push(self.parse_one_const()?);
        }
        Ok(decls)
    }

    fn parse_one_const(&mut self) -> Result<ConstDecl, BasicError> {
        let name = self.expect_ident("a constant name")?;
        let ty = self.parse_as_type()?;
        self.expect(&TokenKind::Eq, "`=` in a Const declaration")?;
        let value = self.parse_expr()?;
        Ok(ConstDecl { name, ty, value })
    }

    /// Consumes an `End <kw>` block terminator.
    pub(super) fn expect_end(&mut self, kw: &str) -> Result<(), BasicError> {
        if !self.eat_kw("End") {
            return Err(self.error(&format!("expected `End {kw}`")));
        }
        if !self.eat_kw(kw) {
            return Err(self.error(&format!("expected `{kw}` after `End`")));
        }
        self.end_of_statement()
    }
}
