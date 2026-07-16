// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The abstract syntax tree shared by both dialects.
//!
//! One AST serves VBA and `StarBasic`; the parser recognises both grammars and
//! the evaluator applies dialect-specific semantics. The tree is organised as
//! expressions ([`expr`]), statements ([`stmt`]), shared declarations
//! ([`decl`]), and the top-level program structure ([`program`]).

pub mod decl;
pub mod expr;
pub mod program;
pub mod stmt;

pub use decl::{ArrayBound, ConstDecl, Param, TypeRef, VarDecl};
pub use expr::{Argument, BinOp, Expr, UnOp};
pub use program::{EnumDef, Item, Module, ModuleOptions, ProcKind, Procedure, TypeDef, Visibility};
pub use stmt::{CaseClause, CaseCond, CompareOp, DoCond, ExitKind, OnError, ResumeKind, Stmt};
