// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Recursive-descent parser and evaluator over the formula token stream.

use std::collections::HashSet;

use loki_basic::Value;
use loki_macro_host::UdfOutcome;
use loki_sheet_model::Workbook;

use super::super::cell_ref::parse_cell_ref;
use super::evaluate_cell;
use super::funcs::{Arg, dispatch, flatten, is_builtin, value_to_cell};
use super::lexer::Token;
use super::{CellValue, FormulaError, UdfResolver};

/// Parses and evaluates a token stream to a number (the arithmetic evaluator's
/// working type). A user-defined function that returns text is `#VALUE!` here;
/// a whole-formula text UDF is handled by [`super::funcs::try_udf_value_formula`].
pub(super) fn evaluate_tokens(
    tokens: Vec<Token>,
    wb: &Workbook,
    visited: &mut HashSet<(usize, usize)>,
    udf: Option<&UdfResolver>,
) -> Result<f64, FormulaError> {
    let mut p = Parser {
        tokens,
        pos: 0,
        wb,
        visited,
        udf,
    };
    let value = p.parse_expr()?;
    if p.pos != p.tokens.len() {
        return Err(FormulaError::Value); // trailing tokens
    }
    Ok(value)
}

pub(super) struct Parser<'a, 'b, 'c> {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) wb: &'a Workbook,
    pub(super) visited: &'b mut HashSet<(usize, usize)>,
    pub(super) udf: Option<&'c UdfResolver>,
}

impl Parser<'_, '_, '_> {
    fn peek_at(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// Resolves an A1 cell to its numeric value, `None` if empty/non-numeric, or
    /// propagates a referenced error value (e.g. a reference cycle's `#REF!`).
    fn resolve(&mut self, row: usize, col: usize) -> Result<Option<f64>, FormulaError> {
        let s = evaluate_cell(row, col, self.wb, &mut *self.visited, self.udf);
        if s.is_empty() {
            return Ok(None);
        }
        if let Some(code) = FormulaError::from_code(&s) {
            return Err(code);
        }
        Ok(s.parse::<f64>().ok())
    }

    fn collect_range(
        &mut self,
        r1: usize,
        c1: usize,
        r2: usize,
        c2: usize,
    ) -> Result<Vec<f64>, FormulaError> {
        let mut out = Vec::new();
        for r in r1.min(r2)..=r1.max(r2) {
            for c in c1.min(c2)..=c1.max(c2) {
                if let Some(v) = self.resolve(r, c)? {
                    out.push(v);
                }
            }
        }
        Ok(out)
    }

    fn parse_expr(&mut self) -> Result<f64, FormulaError> {
        let mut acc = self.parse_term()?;
        while let Some(op) = self.peek_at(0) {
            match op {
                Token::Plus => {
                    self.pos += 1;
                    acc += self.parse_term()?;
                }
                Token::Minus => {
                    self.pos += 1;
                    acc -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(acc)
    }

    fn parse_term(&mut self) -> Result<f64, FormulaError> {
        let mut acc = self.parse_factor()?;
        while let Some(op) = self.peek_at(0) {
            match op {
                Token::Star => {
                    self.pos += 1;
                    acc *= self.parse_factor()?;
                }
                Token::Slash => {
                    self.pos += 1;
                    let d = self.parse_factor()?;
                    if d == 0.0 {
                        return Err(FormulaError::Div0);
                    }
                    acc /= d;
                }
                _ => break,
            }
        }
        Ok(acc)
    }

    fn parse_factor(&mut self) -> Result<f64, FormulaError> {
        match self.peek_at(0).cloned() {
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(-self.parse_factor()?)
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.parse_factor()
            }
            Some(Token::Num(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(Token::LParen) => {
                self.pos += 1;
                let v = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(v)
            }
            Some(Token::Ident(name)) => {
                if self.peek_at(1) == Some(&Token::LParen) {
                    self.parse_function(&name)
                } else {
                    self.pos += 1;
                    let (r, c) = parse_cell_ref(&name).ok_or(FormulaError::Name)?;
                    Ok(self.resolve(r, c)?.unwrap_or(0.0))
                }
            }
            _ => Err(FormulaError::Value),
        }
    }

    fn parse_function(&mut self, name: &str) -> Result<f64, FormulaError> {
        self.pos += 2; // consume Ident and '('
        if name.eq_ignore_ascii_case("if") {
            return self.parse_if_function();
        }
        let args = self.collect_args()?;
        self.expect(Token::RParen)?;
        if is_builtin(name) {
            return dispatch(name, &args);
        }
        // A user-defined function in a numeric context: a text result cannot
        // participate in arithmetic, so it is `#VALUE!` (a whole-formula text
        // UDF is surfaced by `try_udf_value_formula` before reaching here).
        match self.call_udf(name, &args)? {
            CellValue::Num(n) => Ok(n),
            CellValue::Text(_) => Err(FormulaError::Value),
        }
    }

    /// Parses a comma-separated argument list up to (but not consuming) the `)`.
    pub(super) fn collect_args(&mut self) -> Result<Vec<Arg>, FormulaError> {
        let mut args = Vec::new();
        if self.peek_at(0) != Some(&Token::RParen) {
            loop {
                args.push(self.parse_argument()?);
                match self.peek_at(0) {
                    Some(Token::Comma) => self.pos += 1,
                    _ => break,
                }
            }
        }
        Ok(args)
    }

    /// Evaluates the user-defined function `name` with numeric arguments,
    /// compute-only (macro spec §6.3). An unknown name is `#NAME?`; a sandbox
    /// violation / error / budget exhaustion is `#MACRO!`.
    pub(super) fn call_udf(&self, name: &str, args: &[Arg]) -> Result<CellValue, FormulaError> {
        let resolver = self.udf.ok_or(FormulaError::Name)?;
        let values: Vec<Value> = flatten(args).into_iter().map(Value::Double).collect();
        match resolver.call(name, values) {
            Some(UdfOutcome::Value(v)) => value_to_cell(v),
            Some(UdfOutcome::Macro) => Err(FormulaError::Macro),
            None => Err(FormulaError::Name),
        }
    }

    /// Evaluates `IF(cond, then, else)` lazily: only the taken branch is
    /// evaluated, so the untaken branch may reference cells that would error.
    /// Exactly three arguments are required; a missing or extra argument is a
    /// `#VALUE!` error.
    fn parse_if_function(&mut self) -> Result<f64, FormulaError> {
        let scalar = |a: Arg| match a {
            Arg::Scalar(v) => Ok(v),
            Arg::Range(vs) if vs.len() == 1 => Ok(vs[0]),
            _ => Err(FormulaError::Value),
        };
        let cond = scalar(self.parse_argument()?)?;
        self.expect(Token::Comma)?;
        let result = if cond != 0.0 {
            let then_val = scalar(self.parse_argument()?)?;
            self.expect(Token::Comma)?;
            self.skip_argument();
            then_val
        } else {
            self.skip_argument();
            self.expect(Token::Comma)?;
            scalar(self.parse_argument()?)?
        };
        self.expect(Token::RParen)?;
        Ok(result)
    }

    /// Advances `pos` past the current argument without evaluating it,
    /// stopping before a `,` or `)` at depth 0.
    fn skip_argument(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek_at(0) {
                Some(Token::LParen) => {
                    depth += 1;
                    self.pos += 1;
                }
                Some(Token::RParen) if depth > 0 => {
                    depth -= 1;
                    self.pos += 1;
                }
                Some(Token::Comma) if depth == 0 => break,
                Some(Token::RParen) | None => break,
                _ => {
                    self.pos += 1;
                }
            }
        }
    }

    /// Parses one function argument: a range (`A1:B2`), a bare cell reference, or
    /// a scalar expression.
    fn parse_argument(&mut self) -> Result<Arg, FormulaError> {
        if let Some(Token::Ident(s)) = self.peek_at(0) {
            let s = s.clone();
            if let Some((r1, c1)) = parse_cell_ref(&s) {
                match self.peek_at(1) {
                    Some(Token::Colon) => {
                        let end = match self.peek_at(2) {
                            Some(Token::Ident(s2)) => parse_cell_ref(s2),
                            _ => None,
                        };
                        let (r2, c2) = end.ok_or(FormulaError::Value)?;
                        self.pos += 3;
                        return Ok(Arg::Range(self.collect_range(r1, c1, r2, c2)?));
                    }
                    Some(Token::Comma) | Some(Token::RParen) => {
                        // A lone cell reference: a 1-cell range so empty cells
                        // are excluded from COUNT/AVERAGE.
                        self.pos += 1;
                        return Ok(Arg::Range(self.resolve(r1, c1)?.into_iter().collect()));
                    }
                    _ => {}
                }
            }
        }
        Ok(Arg::Scalar(self.parse_expr()?))
    }

    pub(super) fn expect(&mut self, tok: Token) -> Result<(), FormulaError> {
        if self.peek_at(0) == Some(&tok) {
            self.pos += 1;
            Ok(())
        } else {
            Err(FormulaError::Value)
        }
    }
}
