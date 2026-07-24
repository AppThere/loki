// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Compound-statement parsing: `If`/`For`/`Do`/`While`/`Select Case`/`With`.
//! Split from [`super::stmt`] for the 300-line ceiling.

use super::Parser;
use crate::ast::{CaseClause, CaseCond, CompareOp, DoCond, Stmt};
use crate::error::BasicError;
use crate::lexer::TokenKind;

impl Parser {
    /// `If … Then …` — both the block and single-line forms.
    pub(super) fn parse_if(&mut self) -> Result<Stmt, BasicError> {
        self.bump(); // If
        let cond = self.parse_expr()?;
        if !self.eat_kw("Then") {
            return Err(self.error("expected `Then` after the `If` condition"));
        }
        if self.at_stmt_end() {
            return self.parse_block_if(cond);
        }
        // Single-line: `If c Then <stmts> [Else <stmts>]`.
        let then_body = self.parse_inline_stmts(true)?;
        let else_body = if self.eat_kw("Else") {
            Some(self.parse_inline_stmts(false)?)
        } else {
            None
        };
        Ok(Stmt::If {
            branches: vec![(cond, then_body)],
            else_body,
        })
    }

    fn parse_block_if(&mut self, cond: crate::ast::Expr) -> Result<Stmt, BasicError> {
        self.end_of_statement()?;
        let is_end = |p: &Parser| p.peek_kw("ElseIf") || p.peek_kw("Else") || p.peek_kw("End");
        let mut branches = vec![(cond, self.parse_block(&is_end)?)];
        while self.eat_kw("ElseIf") {
            let c = self.parse_expr()?;
            if !self.eat_kw("Then") {
                return Err(self.error("expected `Then` after `ElseIf`"));
            }
            self.end_of_statement()?;
            branches.push((c, self.parse_block(&is_end)?));
        }
        let else_body = if self.eat_kw("Else") {
            self.end_of_statement()?;
            Some(self.parse_block(&|p| p.peek_kw("End"))?)
        } else {
            None
        };
        self.expect_end("If")?;
        Ok(Stmt::If {
            branches,
            else_body,
        })
    }

    /// A `:`-separated run of simple statements on one line (single-line If).
    fn parse_inline_stmts(&mut self, stop_at_else: bool) -> Result<Vec<Stmt>, BasicError> {
        let mut stmts = Vec::new();
        loop {
            if self.at_stmt_end() || (stop_at_else && self.peek_kw("Else")) {
                break;
            }
            stmts.push(self.parse_statement()?);
            if self.eat(&TokenKind::Colon) {
                continue;
            }
            break;
        }
        Ok(stmts)
    }

    /// `For … Next` and `For Each … Next`.
    pub(super) fn parse_for(&mut self) -> Result<Stmt, BasicError> {
        self.bump(); // For
        if self.eat_kw("Each") {
            let var = self.expect_ident("a loop variable")?;
            if !self.eat_kw("In") {
                return Err(self.error("expected `In` in `For Each`"));
            }
            let collection = self.parse_expr()?;
            self.end_of_statement()?;
            let body = self.parse_block(&|p| p.peek_kw("Next"))?;
            self.finish_next()?;
            return Ok(Stmt::ForEach {
                var,
                collection,
                body,
            });
        }
        let var = self.expect_ident("a loop variable")?;
        self.expect(&TokenKind::Eq, "`=` in a For loop")?;
        let from = self.parse_expr()?;
        if !self.eat_kw("To") {
            return Err(self.error("expected `To` in a For loop"));
        }
        let to = self.parse_expr()?;
        let step = if self.eat_kw("Step") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.end_of_statement()?;
        let body = self.parse_block(&|p| p.peek_kw("Next"))?;
        self.finish_next()?;
        Ok(Stmt::For {
            var,
            from,
            to,
            step,
            body,
        })
    }

    /// Consumes `Next` and an optional trailing loop-variable name.
    fn finish_next(&mut self) -> Result<(), BasicError> {
        if !self.eat_kw("Next") {
            return Err(self.error("expected `Next` to close the loop"));
        }
        let _ = self.eat_ident(); // optional loop variable
        Ok(())
    }

    /// `Do … Loop`, with an optional pre- or post-condition.
    pub(super) fn parse_do(&mut self) -> Result<Stmt, BasicError> {
        self.bump(); // Do
        let pre = self.parse_do_cond();
        self.end_of_statement()?;
        let body = self.parse_block(&|p| p.peek_kw("Loop"))?;
        if !self.eat_kw("Loop") {
            return Err(self.error("expected `Loop` to close a `Do`"));
        }
        let post = if pre.is_none() {
            self.parse_do_cond()
        } else {
            None
        };
        Ok(Stmt::DoLoop { pre, post, body })
    }

    fn parse_do_cond(&mut self) -> Option<DoCond> {
        if self.eat_kw("While") {
            self.parse_expr().ok().map(DoCond::While)
        } else if self.eat_kw("Until") {
            self.parse_expr().ok().map(DoCond::Until)
        } else {
            None
        }
    }

    /// `While … Wend`.
    pub(super) fn parse_while(&mut self) -> Result<Stmt, BasicError> {
        self.bump(); // While
        let cond = self.parse_expr()?;
        self.end_of_statement()?;
        let body = self.parse_block(&|p| p.peek_kw("Wend"))?;
        if !self.eat_kw("Wend") {
            return Err(self.error("expected `Wend` to close a `While`"));
        }
        Ok(Stmt::While { cond, body })
    }

    /// `With object … End With`.
    pub(super) fn parse_with(&mut self) -> Result<Stmt, BasicError> {
        self.bump(); // With
        let object = self.parse_expr()?;
        self.end_of_statement()?;
        let body = self.parse_block(&|p| p.peek_kw("End"))?;
        self.expect_end("With")?;
        Ok(Stmt::With { object, body })
    }

    /// `Select Case … End Select`.
    pub(super) fn parse_select(&mut self) -> Result<Stmt, BasicError> {
        self.bump(); // Select
        if !self.eat_kw("Case") {
            return Err(self.error("expected `Case` after `Select`"));
        }
        let subject = self.parse_expr()?;
        self.end_of_statement()?;
        self.skip_terminators();

        let mut cases = Vec::new();
        let mut else_body = None;
        while !self.at_eof() && !self.peek_kw("End") {
            if !self.eat_kw("Case") {
                return Err(self.error("expected `Case` inside `Select Case`"));
            }
            if self.eat_kw("Else") {
                self.end_of_statement()?;
                else_body = Some(self.parse_block(&|p| p.peek_kw("End"))?);
                break;
            }
            let conditions = self.parse_case_conditions()?;
            self.end_of_statement()?;
            let body = self.parse_block(&|p| p.peek_kw("Case") || p.peek_kw("End"))?;
            cases.push(CaseClause { conditions, body });
            self.skip_terminators();
        }
        self.expect_end("Select")?;
        Ok(Stmt::SelectCase {
            subject,
            cases,
            else_body,
        })
    }

    fn parse_case_conditions(&mut self) -> Result<Vec<CaseCond>, BasicError> {
        let mut conds = vec![self.parse_case_cond()?];
        while self.eat(&TokenKind::Comma) {
            conds.push(self.parse_case_cond()?);
        }
        Ok(conds)
    }

    fn parse_case_cond(&mut self) -> Result<CaseCond, BasicError> {
        if self.eat_kw("Is") {
            let op = self.parse_compare_op()?;
            return Ok(CaseCond::Compare(op, self.parse_expr()?));
        }
        let first = self.parse_expr()?;
        if self.eat_kw("To") {
            Ok(CaseCond::Range(first, self.parse_expr()?))
        } else {
            Ok(CaseCond::Value(first))
        }
    }

    fn parse_compare_op(&mut self) -> Result<CompareOp, BasicError> {
        let op = match self.peek_kind() {
            TokenKind::Eq => CompareOp::Eq,
            TokenKind::Ne => CompareOp::Ne,
            TokenKind::Lt => CompareOp::Lt,
            TokenKind::Le => CompareOp::Le,
            TokenKind::Gt => CompareOp::Gt,
            TokenKind::Ge => CompareOp::Ge,
            _ => return Err(self.error("expected a comparison operator after `Is`")),
        };
        self.bump();
        Ok(op)
    }
}
