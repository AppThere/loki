// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Conversion built-ins: `CInt`/`CLng`/`CDbl`/`CBool`/`CStr`, `Val`, `Str`,
//! `Hex`, `Oct`.

use super::{arg, require};
use crate::error::RuntimeError;
use crate::value::Value;

pub(super) fn call(name: &str, a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 1)?;
    let x = arg(a, 0);
    match name {
        "cint" => Ok(Value::Int(x.to_i16()?)),
        "clng" => Ok(Value::Long(x.to_i32()?)),
        "cdbl" => Ok(Value::Double(x.to_f64()?)),
        "cbool" => Ok(Value::Bool(x.to_bool()?)),
        "cstr" => Ok(Value::Str(x.to_basic_string()?)),
        "val" => Ok(Value::Double(val(&x.to_basic_string().unwrap_or_default()))),
        "str" => Ok(Value::Str(str_of(x.to_f64()?))),
        "hex" => Ok(Value::Str(format!("{:X}", x.to_i32()?))),
        "oct" => Ok(Value::Str(format!("{:o}", x.to_i32()?))),
        _ => Err(RuntimeError::invalid_call()),
    }
}

/// `Str(n)`: a leading space stands in for the sign of a non-negative number.
fn str_of(n: f64) -> String {
    let body = Value::Double(n).to_basic_string().unwrap_or_default();
    if n >= 0.0 { format!(" {body}") } else { body }
}

/// `Val`: parses the longest leading numeric prefix, ignoring embedded spaces
/// (VBA `Val("1 2")` is `12`); non-numeric input yields `0`.
fn val(s: &str) -> f64 {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = compact.as_bytes();
    let mut end = 0;
    let mut seen_dot = false;
    let mut seen_exp = false;
    while end < bytes.len() {
        let c = bytes[end] as char;
        let ok = match c {
            '0'..='9' => true,
            '+' | '-' => end == 0 || matches!(bytes[end - 1] as char, 'e' | 'E'),
            '.' if !seen_dot && !seen_exp => {
                seen_dot = true;
                true
            }
            'e' | 'E' if !seen_exp && end > 0 => {
                seen_exp = true;
                true
            }
            _ => false,
        };
        if ok {
            end += 1;
        } else {
            break;
        }
    }
    compact[..end].parse::<f64>().unwrap_or(0.0)
}
