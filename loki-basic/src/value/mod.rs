// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The dynamic [`Value`] type — a BASIC `Variant` — and its coercions and
//! operators.
//!
//! The value model is the heart of a Variant-typed interpreter: values carry
//! their runtime type and operators coerce as VBA/`StarBasic` do (numeric
//! promotion, `Null` propagation, the string-vs-numeric `+` overload). Numeric
//! storage is modelled with the common intrinsic types; `Single`/`Currency`
//! collapse onto [`Value::Double`] for Phase 2 (a documented simplification —
//! macros rarely depend on `Single` precision or `Currency` scaling).

mod array;
mod coerce;
mod ops;
mod ops_logic;

pub use array::Array;
pub use ops::{binary_op, unary_op};

use crate::host::ObjectRef;

/// A dynamically-typed BASIC value (a `Variant`).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// `Empty` — an uninitialised variable. Behaves as `0` or `""` by context.
    #[default]
    Empty,
    /// `Null` — propagates through most operators (SQL-style).
    Null,
    /// A `Boolean` (`True` = `-1`, `False` = `0` numerically).
    Bool(bool),
    /// A 16-bit `Integer`.
    Int(i16),
    /// A 32-bit `Long`.
    Long(i32),
    /// A `Double` (also the storage for `Single`/`Currency` in Phase 2).
    Double(f64),
    /// A `String`.
    Str(String),
    /// A `Date`, stored as an OLE automation date (days since 1899-12-30).
    Date(f64),
    /// An array (value-typed: assignment copies).
    Array(Array),
    /// A reference to a host object (spec §4.3). The interpreter treats it as an
    /// opaque handle: it flows through variables, `Set`, and `With`, compares by
    /// identity for `Is`, and dispatches member access back to the host. It has
    /// no numeric or string value (coercing one is a type mismatch).
    Object(ObjectRef),
}

impl Value {
    /// A `Long` or `Integer` value chosen to fit `n`.
    #[must_use]
    pub fn from_i64_fit(n: i64) -> Value {
        if let Ok(i) = i16::try_from(n) {
            Value::Int(i)
        } else if let Ok(l) = i32::try_from(n) {
            Value::Long(l)
        } else {
            Value::Double(n as f64)
        }
    }

    /// `true` if this value is `Null`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// `true` if this value is `Empty`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }

    /// The VBA `TypeName`/`VarType`-style name of this value's type.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Empty => "Empty",
            Value::Null => "Null",
            Value::Bool(_) => "Boolean",
            Value::Int(_) => "Integer",
            Value::Long(_) => "Long",
            Value::Double(_) => "Double",
            Value::Str(_) => "String",
            Value::Date(_) => "Date",
            Value::Array(_) => "Variant()",
            Value::Object(_) => "Object",
        }
    }

    /// `true` if this value is a host-object reference.
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }
}
