// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Expression AST.
//!
//! One expression tree serves both dialects; dialect-specific operator
//! semantics are applied by the evaluator, not encoded here. Function calls
//! and array indexing share the [`Expr::Call`] node because BASIC spells them
//! identically (`f(1)`); the evaluator resolves which one a name denotes.

use crate::error::Span;

/// A BASIC expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Integer literal.
    Int(i64),
    /// Floating-point literal.
    Float(f64),
    /// String literal.
    Str(String),
    /// Boolean literal (`True` / `False`).
    Bool(bool),
    /// Date literal — the raw text between `#…#`, parsed by the value layer.
    Date(String),
    /// `Empty` — an uninitialised `Variant`.
    Empty,
    /// `Null` — the SQL-style propagating null `Variant`.
    Null,
    /// `Nothing` — a null object reference.
    Nothing,
    /// A variable / parameter / procedure reference by name.
    Var(String),
    /// A prefix operator applied to one operand.
    Unary {
        /// The operator.
        op: UnOp,
        /// The operand.
        operand: Box<Expr>,
    },
    /// A binary operator applied to two operands.
    Binary {
        /// The operator.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// `callee(args)` — a function call **or** an array index (resolved by the
    /// evaluator). `args` may contain omitted slots (`f(1, , 3)`).
    Call {
        /// The thing being called or indexed.
        callee: Box<Expr>,
        /// The argument list.
        args: Vec<Argument>,
    },
    /// `object.member` — member access.
    Member {
        /// The receiver expression.
        object: Box<Expr>,
        /// The member name.
        name: String,
    },
    /// `New ClassName` — object construction (class modules; later phases wire
    /// the object model). Parsed now so programs using it are not rejected.
    New(String),
    /// The implicit receiver of a leading-dot member access inside a `With`
    /// block (`.Name` → `Member { object: WithContext, name: "Name" }`).
    WithContext,
}

/// One argument in a call: positional or named, possibly omitted.
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    /// `Some(name)` for a named argument (`name:=value`); `None` for
    /// positional.
    pub name: Option<String>,
    /// `Some(expr)` for a supplied value; `None` for an omitted slot
    /// (`f(1, , 3)`).
    pub value: Option<Expr>,
}

impl Argument {
    /// A positional argument carrying a value.
    #[must_use]
    pub fn positional(value: Expr) -> Self {
        Self {
            name: None,
            value: Some(value),
        }
    }
}

/// Prefix (unary) operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// Arithmetic negation `-x`.
    Neg,
    /// Unary plus `+x` (identity).
    Pos,
    /// Logical/bitwise `Not x`.
    Not,
}

/// Binary operators, grouped by the precedence the parser assigns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `^` exponentiation.
    Pow,
    /// `*` multiplication.
    Mul,
    /// `/` floating division.
    Div,
    /// `\` integer division.
    IntDiv,
    /// `Mod` remainder.
    Mod,
    /// `+` addition.
    Add,
    /// `-` subtraction.
    Sub,
    /// `&` string concatenation.
    Concat,
    /// `=` equality.
    Eq,
    /// `<>` inequality.
    Ne,
    /// `<` less-than.
    Lt,
    /// `<=` less-or-equal.
    Le,
    /// `>` greater-than.
    Gt,
    /// `>=` greater-or-equal.
    Ge,
    /// `Is` object-identity comparison.
    Is,
    /// `Like` pattern match.
    Like,
    /// `And` logical/bitwise and.
    And,
    /// `Or` logical/bitwise or.
    Or,
    /// `Xor` logical/bitwise exclusive-or.
    Xor,
    /// `Eqv` logical/bitwise equivalence.
    Eqv,
    /// `Imp` logical/bitwise implication.
    Imp,
}

/// An expression paired with its source span (used where diagnostics need a
/// location, e.g. call sites).
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    /// The expression.
    pub expr: Expr,
    /// Its source location.
    pub span: Span,
}
