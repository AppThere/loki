// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared declaration AST: type references, variable declarations, array
//! bounds, constants, and procedure parameters.

use super::expr::Expr;

/// A declared type (`As …`). Absence of an `As` clause is [`TypeRef::Implicit`]
/// (a `Variant`).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    /// No `As` clause — a `Variant`.
    Implicit,
    /// A named type: an intrinsic (`Integer`, `Long`, `Single`, `Double`,
    /// `Currency`, `String`, `Boolean`, `Date`, `Object`, `Variant`) or a
    /// user-defined `Type` / class name. Case-insensitive; stored verbatim.
    Named(String),
    /// A fixed-length string `String * N`.
    FixedString(usize),
}

/// One array dimension bound. `lower == None` means "use `Option Base`".
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayBound {
    /// Lower bound expression, or `None` to use the module's `Option Base`.
    pub lower: Option<Expr>,
    /// Upper bound expression.
    pub upper: Expr,
}

/// A single variable declaration (one name in a `Dim`/`ReDim`/`Static`
/// statement or module-level `Public`/`Private`).
#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    /// The variable name.
    pub name: String,
    /// Its declared type.
    pub ty: TypeRef,
    /// `Some(bounds)` if declared as an array (possibly empty for a
    /// dynamic array `Dim a()`).
    pub bounds: Option<Vec<ArrayBound>>,
}

/// A `Const` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    /// The constant name.
    pub name: String,
    /// Its declared type.
    pub ty: TypeRef,
    /// The constant value expression (evaluated once).
    pub value: Expr,
}

/// A procedure parameter.
// Four independent BASIC parameter modifiers (ByVal/Optional/ParamArray/array);
// a plain flag carrier, not a state machine the lint is meant to catch.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// The parameter name.
    pub name: String,
    /// `true` for `ByVal`; `false` (the default) for `ByRef`.
    pub by_val: bool,
    /// `true` if declared `Optional`.
    pub optional: bool,
    /// `true` if declared `ParamArray` (variadic; collects the tail).
    pub param_array: bool,
    /// Declared as an array parameter (`name()`).
    pub is_array: bool,
    /// The declared type.
    pub ty: TypeRef,
    /// Default value for an `Optional` parameter, if any.
    pub default: Option<Expr>,
}
