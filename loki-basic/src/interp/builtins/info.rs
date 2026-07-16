// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Array-bound and type-inspection built-ins: `LBound`, `UBound`, `IsNumeric`,
//! `IsEmpty`, `IsNull`, `IsArray`, `TypeName`, and `IIf`.

use super::{arg, require};
use crate::error::RuntimeError;
use crate::value::Value;

pub(super) fn call(name: &str, a: &[Value]) -> Result<Value, RuntimeError> {
    match name {
        "lbound" | "ubound" => bound(name, a),
        "isnumeric" => Ok(Value::Bool(is_numeric(&arg(a, 0)))),
        "isempty" => Ok(Value::Bool(matches!(arg(a, 0), Value::Empty))),
        "isnull" => Ok(Value::Bool(matches!(arg(a, 0), Value::Null))),
        "isarray" => Ok(Value::Bool(matches!(arg(a, 0), Value::Array(_)))),
        "typename" => Ok(Value::Str(arg(a, 0).type_name().to_string())),
        "iif" => iif(a),
        _ => Err(RuntimeError::invalid_call()),
    }
}

fn bound(name: &str, a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 1)?;
    let Value::Array(arr) = arg(a, 0) else {
        return Err(RuntimeError::type_mismatch());
    };
    let dim = if a.len() >= 2 {
        usize::try_from(arg(a, 1).to_i32()?).unwrap_or(0)
    } else {
        1
    };
    let b = if name == "lbound" {
        arr.lbound(dim)
    } else {
        arr.ubound(dim)
    };
    b.map(Value::Long)
        .ok_or_else(RuntimeError::subscript_out_of_range)
}

fn is_numeric(v: &Value) -> bool {
    match v {
        Value::Int(_) | Value::Long(_) | Value::Double(_) | Value::Date(_) | Value::Bool(_) => true,
        Value::Str(s) => s.trim().parse::<f64>().is_ok(),
        Value::Empty | Value::Null | Value::Array(_) => false,
    }
}

fn iif(a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 3)?;
    if arg(a, 0).to_bool()? {
        Ok(arg(a, 1))
    } else {
        Ok(arg(a, 2))
    }
}
