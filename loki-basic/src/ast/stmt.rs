// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Statement AST.

use super::decl::{ConstDecl, VarDecl};
use super::expr::Expr;

/// A statement inside a procedure body.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `Dim`/`Static`/`Private`/`Public` local declarations.
    Dim(Vec<VarDecl>),
    /// `ReDim [Preserve] a(bounds)`.
    ReDim {
        /// Whether `Preserve` was given.
        preserve: bool,
        /// The (re)declared arrays.
        decls: Vec<VarDecl>,
    },
    /// `Const` local declarations.
    Const(Vec<ConstDecl>),
    /// `[Let] target = value` — value assignment.
    Assign {
        /// The assignment target (variable, array element, or member).
        target: Expr,
        /// The right-hand side.
        value: Expr,
    },
    /// `Set target = value` — object-reference assignment.
    Set {
        /// The assignment target.
        target: Expr,
        /// The right-hand side (an object reference).
        value: Expr,
    },
    /// A procedure call statement (`Call f(x)` or bare `f x`).
    Call(Expr),
    /// `If … Then … [ElseIf …] [Else …] End If` (block or single-line).
    If {
        /// `(condition, body)` for the `If` and each `ElseIf`, in order.
        branches: Vec<(Expr, Vec<Stmt>)>,
        /// The optional `Else` body.
        else_body: Option<Vec<Stmt>>,
    },
    /// `Select Case subject … End Select`.
    SelectCase {
        /// The value being matched.
        subject: Expr,
        /// The `Case` clauses.
        cases: Vec<CaseClause>,
        /// The optional `Case Else` body.
        else_body: Option<Vec<Stmt>>,
    },
    /// `For var = from To to [Step step] … Next`.
    For {
        /// Loop counter variable name.
        var: String,
        /// Initial value.
        from: Expr,
        /// Terminal value.
        to: Expr,
        /// Step expression (defaults to `1`).
        step: Option<Expr>,
        /// Loop body.
        body: Vec<Stmt>,
    },
    /// `For Each var In collection … Next`.
    ForEach {
        /// Iteration variable name.
        var: String,
        /// The collection/array expression.
        collection: Expr,
        /// Loop body.
        body: Vec<Stmt>,
    },
    /// `Do … Loop` with an optional pre- or post-condition.
    DoLoop {
        /// A `While`/`Until` guard on the `Do`, if any.
        pre: Option<DoCond>,
        /// A `While`/`Until` guard on the `Loop`, if any.
        post: Option<DoCond>,
        /// Loop body.
        body: Vec<Stmt>,
    },
    /// `While cond … Wend`.
    While {
        /// The loop condition.
        cond: Expr,
        /// Loop body.
        body: Vec<Stmt>,
    },
    /// `With object … End With`.
    With {
        /// The object expression bound to leading-dot member access.
        object: Expr,
        /// The `With` body.
        body: Vec<Stmt>,
    },
    /// An `Exit …` statement.
    Exit(ExitKind),
    /// `GoTo label`.
    GoTo(String),
    /// A statement label (`label:` or a bare numeric line label).
    Label(String),
    /// `On Error …`.
    OnError(OnError),
    /// `Resume [0 | Next | label]`.
    Resume(ResumeKind),
    /// `Error n` — raise error number `n`.
    ErrorStmt(Expr),
    /// `End` or `Stop` — halt all execution.
    Halt,
    /// An empty statement (blank line / stray separator).
    Empty,
}

/// One `Case` clause of a `Select Case`.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseClause {
    /// The match conditions (any match selects the clause).
    pub conditions: Vec<CaseCond>,
    /// The clause body.
    pub body: Vec<Stmt>,
}

/// A single `Case` condition.
#[derive(Debug, Clone, PartialEq)]
pub enum CaseCond {
    /// `Case value` — equality with the subject.
    Value(Expr),
    /// `Case a To b` — inclusive range.
    Range(Expr, Expr),
    /// `Case Is <op> value` — a comparison against the subject.
    Compare(CompareOp, Expr),
}

/// The comparison operators usable in `Case Is …`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    /// `=`
    Eq,
    /// `<>`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// The guard on a `Do`/`Loop`.
#[derive(Debug, Clone, PartialEq)]
pub enum DoCond {
    /// `While cond` — loop while true.
    While(Expr),
    /// `Until cond` — loop until true.
    Until(Expr),
}

/// Which enclosing construct an `Exit` leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    /// `Exit For`
    For,
    /// `Exit Do`
    Do,
    /// `Exit Sub`
    Sub,
    /// `Exit Function`
    Function,
    /// `Exit Property`
    Property,
}

/// The forms of `On Error`.
#[derive(Debug, Clone, PartialEq)]
pub enum OnError {
    /// `On Error GoTo label`.
    GoToLabel(String),
    /// `On Error GoTo 0` — disable the active handler.
    Disable,
    /// `On Error Resume Next` — continue at the next statement on error.
    ResumeNext,
}

/// The forms of `Resume`.
#[derive(Debug, Clone, PartialEq)]
pub enum ResumeKind {
    /// `Resume` / `Resume 0` — retry the faulting statement.
    Retry,
    /// `Resume Next` — continue after the faulting statement.
    Next,
    /// `Resume label` — continue at a label.
    Label(String),
}
