// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Expression parsing (precedence-climbing / Pratt) for [`Parser`].
//!
//! Precedence, tightest last: `Imp < Eqv < Xor < Or < And < Not(unary) <
//! comparison < & < +/- < Mod < \ < * / < unary +/- < ^`. Exponentiation binds
//! tighter than unary minus, so `-2^2 == -(2^2)`.

use super::Parser;
use crate::ast::{Argument, BinOp, Expr, UnOp};
use crate::error::BasicError;
use crate::lexer::TokenKind;

/// Precedence of the exponent operator — unary `-`/`+` parse their operand at
/// this level so `^` binds into it.
const POW_PREC: u8 = 13;
/// Precedence of the comparison operators — `Not` parses its operand here.
const CMP_PREC: u8 = 6;

impl Parser {
    /// Parses a full expression.
    pub(super) fn parse_expr(&mut self) -> Result<Expr, BasicError> {
        self.parse_bin(0)
    }

    fn parse_bin(&mut self, min_prec: u8) -> Result<Expr, BasicError> {
        let mut lhs = self.parse_unary()?;
        while let Some((op, prec)) = self.binop_here() {
            if prec < min_prec {
                break;
            }
            self.consume_binop();
            let rhs = self.parse_bin(prec + 1)?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, BasicError> {
        let op = match self.peek_kind() {
            TokenKind::Minus => Some((UnOp::Neg, POW_PREC)),
            TokenKind::Plus => Some((UnOp::Pos, POW_PREC)),
            _ if self.peek_kw("Not") => Some((UnOp::Not, CMP_PREC)),
            _ => None,
        };
        if let Some((op, operand_prec)) = op {
            self.bump();
            let operand = self.parse_bin(operand_prec)?;
            return Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
            });
        }
        self.parse_postfix()
    }

    pub(super) fn parse_postfix(&mut self) -> Result<Expr, BasicError> {
        let mut e = self.parse_primary()?;
        loop {
            match self.peek_kind() {
                TokenKind::LParen => {
                    self.bump();
                    let args = self.parse_arguments()?;
                    self.expect(&TokenKind::RParen, "`)` to close arguments")?;
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                    };
                }
                TokenKind::Dot => {
                    self.bump();
                    let name = self.expect_ident("a member name after `.`")?;
                    e = Expr::Member {
                        object: Box::new(e),
                        name,
                    };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, BasicError> {
        match self.peek_kind().clone() {
            TokenKind::Int(n) => {
                self.bump();
                Ok(Expr::Int(n))
            }
            TokenKind::Float(f) => {
                self.bump();
                Ok(Expr::Float(f))
            }
            TokenKind::Str(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            TokenKind::Date(d) => {
                self.bump();
                Ok(Expr::Date(d))
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "`)`")?;
                Ok(inner)
            }
            // Leading-dot member access inside a `With` block.
            TokenKind::Dot => {
                self.bump();
                let name = self.expect_ident("a member name after `.`")?;
                Ok(Expr::Member {
                    object: Box::new(Expr::WithContext),
                    name,
                })
            }
            TokenKind::Ident(name) => self.parse_ident_primary(&name),
            _ => Err(self.error("expected an expression")),
        }
    }

    fn parse_ident_primary(&mut self, name: &str) -> Result<Expr, BasicError> {
        // Keyword literals first.
        if name.eq_ignore_ascii_case("True") {
            self.bump();
            return Ok(Expr::Bool(true));
        }
        if name.eq_ignore_ascii_case("False") {
            self.bump();
            return Ok(Expr::Bool(false));
        }
        if name.eq_ignore_ascii_case("Empty") {
            self.bump();
            return Ok(Expr::Empty);
        }
        if name.eq_ignore_ascii_case("Null") {
            self.bump();
            return Ok(Expr::Null);
        }
        if name.eq_ignore_ascii_case("Nothing") {
            self.bump();
            return Ok(Expr::Nothing);
        }
        if name.eq_ignore_ascii_case("New") {
            self.bump();
            let class = self.expect_ident("a class name after `New`")?;
            return Ok(Expr::New(class));
        }
        self.bump();
        Ok(Expr::Var(name.to_string()))
    }

    /// Parses a comma-separated argument list (already past the `(`), allowing
    /// omitted slots (`f(1, , 3)`) and named arguments (`name:=value`).
    pub(super) fn parse_arguments(&mut self) -> Result<Vec<Argument>, BasicError> {
        let mut args = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_argument()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    pub(super) fn parse_argument(&mut self) -> Result<Argument, BasicError> {
        // Omitted slot: a comma or the closing paren with nothing before it.
        if matches!(self.peek_kind(), TokenKind::Comma | TokenKind::RParen) {
            return Ok(Argument {
                name: None,
                value: None,
            });
        }
        // Named argument `name := value` (Ident, Colon, Eq in sequence).
        if let TokenKind::Ident(n) = self.peek_kind().clone()
            && matches!(self.peek2_kind(), TokenKind::Colon)
            && matches!(self.peek_at(2), TokenKind::Eq)
        {
            self.bump(); // name
            self.bump(); // ':'
            self.bump(); // '='
            let value = self.parse_expr()?;
            return Ok(Argument {
                name: Some(n),
                value: Some(value),
            });
        }
        let value = self.parse_expr()?;
        Ok(Argument {
            name: None,
            value: Some(value),
        })
    }

    // ── Binary-operator table ───────────────────────────────────────────────

    /// The binary operator at the cursor and its precedence, without consuming.
    fn binop_here(&self) -> Option<(BinOp, u8)> {
        let op = match self.peek_kind() {
            TokenKind::Caret => BinOp::Pow,
            TokenKind::Star => BinOp::Mul,
            TokenKind::Slash => BinOp::Div,
            TokenKind::Backslash => BinOp::IntDiv,
            TokenKind::Plus => BinOp::Add,
            TokenKind::Minus => BinOp::Sub,
            TokenKind::Amp => BinOp::Concat,
            TokenKind::Eq => BinOp::Eq,
            TokenKind::Ne => BinOp::Ne,
            TokenKind::Lt => BinOp::Lt,
            TokenKind::Le => BinOp::Le,
            TokenKind::Gt => BinOp::Gt,
            TokenKind::Ge => BinOp::Ge,
            TokenKind::Ident(w) => return keyword_binop(w),
            _ => return None,
        };
        Some((op, precedence(op)))
    }

    fn consume_binop(&mut self) {
        self.bump();
    }
}

fn keyword_binop(word: &str) -> Option<(BinOp, u8)> {
    let op = match () {
        () if word.eq_ignore_ascii_case("Mod") => BinOp::Mod,
        () if word.eq_ignore_ascii_case("Is") => BinOp::Is,
        () if word.eq_ignore_ascii_case("Like") => BinOp::Like,
        () if word.eq_ignore_ascii_case("And") => BinOp::And,
        () if word.eq_ignore_ascii_case("Or") => BinOp::Or,
        () if word.eq_ignore_ascii_case("Xor") => BinOp::Xor,
        () if word.eq_ignore_ascii_case("Eqv") => BinOp::Eqv,
        () if word.eq_ignore_ascii_case("Imp") => BinOp::Imp,
        () => return None,
    };
    Some((op, precedence(op)))
}

fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Imp => 1,
        BinOp::Eqv => 2,
        BinOp::Xor => 3,
        BinOp::Or => 4,
        BinOp::And => 5,
        BinOp::Eq
        | BinOp::Ne
        | BinOp::Lt
        | BinOp::Le
        | BinOp::Gt
        | BinOp::Ge
        | BinOp::Is
        | BinOp::Like => CMP_PREC,
        BinOp::Concat => 7,
        BinOp::Add | BinOp::Sub => 8,
        BinOp::Mod => 9,
        BinOp::IntDiv => 10,
        BinOp::Mul | BinOp::Div => 11,
        BinOp::Pow => POW_PREC,
    }
}
