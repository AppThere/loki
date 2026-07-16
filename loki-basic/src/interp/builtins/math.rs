// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Numeric built-ins: `Abs`, `Sgn`, `Int`, `Fix`, `Sqr`, the trig/exp/log
//! family, and `Round` (banker's rounding).

use super::{arg, require};
use crate::error::RuntimeError;
use crate::value::Value;

pub(super) fn call(name: &str, a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 1)?;
    let x = arg(a, 0);
    match name {
        "abs" => abs(&x),
        "sgn" => {
            let f = x.to_f64()?;
            Ok(Value::Int(f.partial_cmp(&0.0).map_or(0, |o| o as i16)))
        }
        "int" => Ok(Value::Double(x.to_f64()?.floor())),
        "fix" => Ok(Value::Double(x.to_f64()?.trunc())),
        "sqr" => guarded(x.to_f64()?, |v| v >= 0.0, f64::sqrt),
        "sin" => Ok(Value::Double(x.to_f64()?.sin())),
        "cos" => Ok(Value::Double(x.to_f64()?.cos())),
        "tan" => Ok(Value::Double(x.to_f64()?.tan())),
        "atn" => Ok(Value::Double(x.to_f64()?.atan())),
        "exp" => Ok(Value::Double(x.to_f64()?.exp())),
        "log" => guarded(x.to_f64()?, |v| v > 0.0, f64::ln),
        "round" => round(&x, a),
        _ => Err(RuntimeError::invalid_call()),
    }
}

fn abs(x: &Value) -> Result<Value, RuntimeError> {
    if matches!(x, Value::Double(_) | Value::Date(_) | Value::Str(_)) {
        Ok(Value::Double(x.to_f64()?.abs()))
    } else {
        Ok(Value::from_i64_fit(x.to_i64_round()?.abs()))
    }
}

fn guarded(
    v: f64,
    ok: impl Fn(f64) -> bool,
    f: impl Fn(f64) -> f64,
) -> Result<Value, RuntimeError> {
    if ok(v) {
        Ok(Value::Double(f(v)))
    } else {
        Err(RuntimeError::invalid_call())
    }
}

fn round(x: &Value, a: &[Value]) -> Result<Value, RuntimeError> {
    let digits = if a.len() >= 2 {
        arg(a, 1).to_i32()?.max(0)
    } else {
        0
    };
    let factor = 10f64.powi(digits);
    let scaled = x.to_f64()? * factor;
    Ok(Value::Double(round_half_even(scaled) as f64 / factor))
}

/// Banker's rounding (duplicated from the value layer to keep built-ins pure and
/// self-contained).
fn round_half_even(x: f64) -> i64 {
    let floor = x.floor();
    let diff = x - floor;
    let r = if diff < 0.5 {
        floor
    } else if diff > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    };
    r as i64
}
