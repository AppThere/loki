// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Statement parsing: the per-statement dispatch, simple statements, the block
//! reader, and error-handling statements. Compound constructs (If/For/Do/
//! While/Select/With) live in [`super::stmt_block`].

use super::Parser;
use crate::ast::{Argument, ExitKind, Expr, OnError, ResumeKind, Stmt};
use crate::error::BasicError;
use crate::lexer::TokenKind;

impl Parser {
    /// Parses a `{ … }`-style body: statements until `is_end` is true (the
    /// terminating keyword line is left for the caller to consume) or EOF.
    pub(super) fn parse_block(
        &mut self,
        is_end: &dyn Fn(&Parser) -> bool,
    ) -> Result<Vec<Stmt>, BasicError> {
        let mut stmts = Vec::new();
        loop {
            self.skip_terminators();
            if self.at_eof() || is_end(self) {
                break;
            }
            let stmt = self.parse_statement()?;
            let is_label = matches!(stmt, Stmt::Label(_));
            stmts.push(stmt);
            if is_label {
                // A label's `:` (or line boundary) already separated it.
                continue;
            }
            if self.at_eof() || is_end(self) {
                break;
            }
            self.end_of_statement()?;
        }
        Ok(stmts)
    }

    /// Parses a single statement (without consuming its terminator).
    pub(super) fn parse_statement(&mut self) -> Result<Stmt, BasicError> {
        // Numeric line-number label (`10 …`).
        if let TokenKind::Int(n) = *self.peek_kind() {
            self.bump();
            return Ok(Stmt::Label(n.to_string()));
        }
        // Identifier label (`name:`).
        if matches!(self.peek_kind(), TokenKind::Ident(_))
            && matches!(self.peek2_kind(), TokenKind::Colon)
        {
            let name = self.eat_ident().unwrap_or_default();
            self.bump(); // ':'
            return Ok(Stmt::Label(name));
        }

        if let Some(stmt) = self.parse_keyword_statement()? {
            return Ok(stmt);
        }
        self.parse_expr_statement()
    }

    /// Dispatches keyword-led statements. Returns `None` when the statement is
    /// not keyword-led (an assignment or bare call).
    fn parse_keyword_statement(&mut self) -> Result<Option<Stmt>, BasicError> {
        if self.eat_kw("Dim") || self.eat_kw("Static") {
            return Ok(Some(Stmt::Dim(self.parse_var_decls()?)));
        }
        if self.eat_kw("ReDim") {
            let preserve = self.eat_kw("Preserve");
            return Ok(Some(Stmt::ReDim {
                preserve,
                decls: self.parse_var_decls()?,
            }));
        }
        if self.eat_kw("Const") {
            return Ok(Some(Stmt::Const(self.parse_const_decls()?)));
        }
        if self.peek_kw("If") {
            return Ok(Some(self.parse_if()?));
        }
        if self.peek_kw("For") {
            return Ok(Some(self.parse_for()?));
        }
        if self.peek_kw("Do") {
            return Ok(Some(self.parse_do()?));
        }
        if self.peek_kw("While") {
            return Ok(Some(self.parse_while()?));
        }
        if self.peek_kw("Select") {
            return Ok(Some(self.parse_select()?));
        }
        if self.peek_kw("With") {
            return Ok(Some(self.parse_with()?));
        }
        if self.eat_kw("Exit") {
            return Ok(Some(self.parse_exit()?));
        }
        if self.eat_kw("GoTo") {
            return Ok(Some(Stmt::GoTo(self.parse_label_ref()?)));
        }
        if self.eat_kw("On") {
            return Ok(Some(self.parse_on_error()?));
        }
        if self.eat_kw("Resume") {
            return Ok(Some(self.parse_resume()?));
        }
        if self.eat_kw("Error") {
            return Ok(Some(Stmt::ErrorStmt(self.parse_expr()?)));
        }
        if self.eat_kw("Call") {
            return Ok(Some(Stmt::Call(self.parse_postfix()?)));
        }
        if self.eat_kw("Set") {
            let target = self.parse_postfix()?;
            self.expect(&TokenKind::Eq, "`=` in a Set statement")?;
            return Ok(Some(Stmt::Set {
                target,
                value: self.parse_expr()?,
            }));
        }
        if self.eat_kw("Let") {
            let target = self.parse_postfix()?;
            self.expect(&TokenKind::Eq, "`=` in a Let statement")?;
            return Ok(Some(Stmt::Assign {
                target,
                value: self.parse_expr()?,
            }));
        }
        if self.eat_kw("Stop") {
            return Ok(Some(Stmt::Halt));
        }
        if self.peek_kw("End") && self.peek_at_stmt_end_after_one() {
            self.bump(); // End
            return Ok(Some(Stmt::Halt));
        }
        Ok(None)
    }

    /// Assignment (`target = value`) or bare procedure call (`f a, b`).
    fn parse_expr_statement(&mut self) -> Result<Stmt, BasicError> {
        let target = self.parse_postfix()?;
        if self.eat(&TokenKind::Eq) {
            let value = self.parse_expr()?;
            return Ok(Stmt::Assign { target, value });
        }
        if self.at_stmt_end() {
            return Ok(Stmt::Call(target));
        }
        let args = self.parse_bare_args()?;
        Ok(Stmt::Call(Expr::Call {
            callee: Box::new(target),
            args,
        }))
    }

    /// Bare (paren-less) call arguments: arguments (positional, named, or
    /// omitted) separated by `,` or `;`.
    fn parse_bare_args(&mut self) -> Result<Vec<Argument>, BasicError> {
        let mut args = Vec::new();
        loop {
            args.push(self.parse_argument()?);
            if self.eat(&TokenKind::Comma) || self.eat(&TokenKind::Semicolon) {
                if self.at_stmt_end() {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(args)
    }

    fn parse_exit(&mut self) -> Result<Stmt, BasicError> {
        let kind = if self.eat_kw("For") {
            ExitKind::For
        } else if self.eat_kw("Do") {
            ExitKind::Do
        } else if self.eat_kw("Sub") {
            ExitKind::Sub
        } else if self.eat_kw("Function") {
            ExitKind::Function
        } else if self.eat_kw("Property") {
            ExitKind::Property
        } else {
            return Err(self.error("expected For, Do, Sub, Function, or Property after Exit"));
        };
        Ok(Stmt::Exit(kind))
    }

    fn parse_on_error(&mut self) -> Result<Stmt, BasicError> {
        if !self.eat_kw("Error") {
            return Err(self.error("only `On Error …` is supported"));
        }
        if self.eat_kw("Resume") {
            if !self.eat_kw("Next") {
                return Err(self.error("expected `Next` after `On Error Resume`"));
            }
            return Ok(Stmt::OnError(OnError::ResumeNext));
        }
        if self.eat_kw("GoTo") {
            // `On Error GoTo 0` disables; otherwise it names a label.
            if let TokenKind::Int(0) = *self.peek_kind() {
                self.bump();
                return Ok(Stmt::OnError(OnError::Disable));
            }
            return Ok(Stmt::OnError(OnError::GoToLabel(self.parse_label_ref()?)));
        }
        Err(self.error("expected `Resume Next` or `GoTo` after `On Error`"))
    }

    fn parse_resume(&mut self) -> Result<Stmt, BasicError> {
        if self.at_stmt_end() {
            return Ok(Stmt::Resume(ResumeKind::Retry));
        }
        if self.eat_kw("Next") {
            return Ok(Stmt::Resume(ResumeKind::Next));
        }
        if let TokenKind::Int(0) = *self.peek_kind() {
            self.bump();
            return Ok(Stmt::Resume(ResumeKind::Retry));
        }
        Ok(Stmt::Resume(ResumeKind::Label(self.parse_label_ref()?)))
    }

    /// A `GoTo`/`Resume` target: an identifier or a numeric line label.
    pub(super) fn parse_label_ref(&mut self) -> Result<String, BasicError> {
        if let TokenKind::Int(n) = *self.peek_kind() {
            self.bump();
            return Ok(n.to_string());
        }
        self.expect_ident("a label name")
    }

    fn peek_at_stmt_end_after_one(&self) -> bool {
        matches!(
            self.peek2_kind(),
            TokenKind::Newline | TokenKind::Colon | TokenKind::Eof
        )
    }
}
