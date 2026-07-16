// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level program AST: modules, procedures, and module-level declarations.

use super::decl::{ConstDecl, Param, TypeRef, VarDecl};
use super::expr::Expr;
use super::stmt::Stmt;
use crate::dialect::Dialect;

/// A single BASIC module (one code file / component).
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// The module name, if declared (`Attribute VB_Name`), else `None`.
    pub name: Option<String>,
    /// The dialect the module was parsed as (and should be evaluated as).
    pub dialect: Dialect,
    /// Module-level `Option` settings.
    pub options: ModuleOptions,
    /// The module's top-level items in source order.
    pub items: Vec<Item>,
}

/// Module-level `Option` settings. Defaults (`base 0`, no `Explicit`, binary
/// compare) match `#[derive(Default)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModuleOptions {
    /// The array lower-bound default (`Option Base 0` or `1`).
    pub base: i32,
    /// `Option Explicit` — require declaration before use.
    pub explicit: bool,
    /// `Option Compare Text` (case-insensitive string comparison) when `true`;
    /// binary comparison when `false`.
    pub compare_text: bool,
}

/// A top-level item in a module.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A `Sub`/`Function`/`Property` procedure.
    Procedure(Procedure),
    /// A user-defined record `Type … End Type`.
    Type(TypeDef),
    /// An `Enum … End Enum`.
    Enum(EnumDef),
    /// Module-level `Const` declarations.
    Const(Vec<ConstDecl>),
    /// Module-level variable declarations (`Dim`/`Public`/`Private`).
    Var(Vec<VarDecl>),
    /// A `Declare [Function|Sub] … Lib …` foreign (FFI) declaration.
    ///
    /// FFI is on the "never" list (macro spec §7): the declaration is parsed so
    /// valid programs are accepted, but **calling** it raises an untrappable
    /// feature-refusal at runtime.
    ForeignDecl {
        /// The declared procedure name.
        name: String,
    },
}

/// A procedure definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Procedure {
    /// The procedure name.
    pub name: String,
    /// Which kind of procedure this is.
    pub kind: ProcKind,
    /// `Public`/`Private` visibility.
    pub visibility: Visibility,
    /// `true` if declared `Static` (locals persist between calls).
    pub is_static: bool,
    /// The parameter list.
    pub params: Vec<Param>,
    /// The return type (`Function`/`Property Get`); [`TypeRef::Implicit`] for a
    /// `Sub`.
    pub ret_ty: TypeRef,
    /// The procedure body.
    pub body: Vec<Stmt>,
}

/// The kind of a [`Procedure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcKind {
    /// A `Sub` (no return value).
    Sub,
    /// A `Function` (returns a value via the procedure name).
    Function,
    /// A `Property Get` accessor.
    PropertyGet,
    /// A `Property Let` value-setter.
    PropertyLet,
    /// A `Property Set` object-setter.
    PropertySet,
}

/// Procedure/variable visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// `Public` (the default for procedures).
    #[default]
    Public,
    /// `Private`.
    Private,
}

/// A user-defined record type (`Type … End Type`).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    /// The type name.
    pub name: String,
    /// The record fields.
    pub fields: Vec<VarDecl>,
}

/// An enumeration (`Enum … End Enum`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// The enum name.
    pub name: String,
    /// Members as `(name, explicit_value)`; `None` value auto-increments.
    pub members: Vec<(String, Option<Expr>)>,
}
