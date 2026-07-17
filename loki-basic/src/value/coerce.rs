// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Coercions between [`Value`] variants (VBA `CBool`/`CDbl`/`CStr`/`CLng`
//! semantics, and the implicit coercions the operators use).

use super::Value;
use crate::error::RuntimeError;

/// The numeric "kind" an operand contributes to arithmetic promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum NumKind {
    /// `Integer`/`Boolean`/`Empty` — 16-bit integer domain.
    Integer,
    /// `Long` — 32-bit integer domain.
    Long,
    /// `Double`/`Date` — floating domain.
    Double,
}

impl Value {
    /// Coerces to a boolean (`CBool`). Non-zero numbers are `True`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::type_mismatch`] for `Null`, arrays, or non-numeric,
    /// non-boolean strings; error 94 semantics fold into type mismatch here.
    pub fn to_bool(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Empty => Ok(false),
            Value::Bool(b) => Ok(*b),
            Value::Int(_) | Value::Long(_) | Value::Double(_) | Value::Date(_) => {
                Ok(self.to_f64()? != 0.0)
            }
            Value::Str(s) => {
                let t = s.trim();
                if t.eq_ignore_ascii_case("true") {
                    Ok(true)
                } else if t.eq_ignore_ascii_case("false") {
                    Ok(false)
                } else {
                    Ok(parse_number(t)? != 0.0)
                }
            }
            Value::Null | Value::Array(_) | Value::Object(_) => Err(RuntimeError::type_mismatch()),
        }
    }

    /// Coerces to `f64` (`CDbl`). `Empty` is `0`; `Bool` is `-1`/`0`.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::type_mismatch`] for `Null`, arrays, or non-numeric
    /// strings.
    pub fn to_f64(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Empty => Ok(0.0),
            Value::Bool(b) => Ok(if *b { -1.0 } else { 0.0 }),
            Value::Int(i) => Ok(f64::from(*i)),
            Value::Long(l) => Ok(f64::from(*l)),
            Value::Double(d) | Value::Date(d) => Ok(*d),
            Value::Str(s) => parse_number(s.trim()),
            Value::Null | Value::Array(_) | Value::Object(_) => Err(RuntimeError::type_mismatch()),
        }
    }

    /// Coerces to `i64` using banker's rounding (`CLng`-style, half-to-even).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::type_mismatch`] if not numeric-coercible.
    pub fn to_i64_round(&self) -> Result<i64, RuntimeError> {
        Ok(round_half_even(self.to_f64()?))
    }

    /// Coerces to `i32` (`CLng`) with range checking.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::overflow`] if out of `Long` range; type mismatch if not
    /// numeric.
    pub fn to_i32(&self) -> Result<i32, RuntimeError> {
        let n = self.to_i64_round()?;
        i32::try_from(n).map_err(|_| RuntimeError::overflow())
    }

    /// Coerces to `i16` (`CInt`) with range checking.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::overflow`] if out of `Integer` range.
    pub fn to_i16(&self) -> Result<i16, RuntimeError> {
        let n = self.to_i64_round()?;
        i16::try_from(n).map_err(|_| RuntimeError::overflow())
    }

    /// Coerces to a display string (`CStr`).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::type_mismatch`] for `Null` and arrays.
    pub fn to_basic_string(&self) -> Result<String, RuntimeError> {
        match self {
            Value::Empty => Ok(String::new()),
            Value::Bool(b) => Ok(if *b { "True".into() } else { "False".into() }),
            Value::Int(i) => Ok(i.to_string()),
            Value::Long(l) => Ok(l.to_string()),
            Value::Double(d) | Value::Date(d) => Ok(format_number(*d)),
            Value::Str(s) => Ok(s.clone()),
            Value::Null | Value::Array(_) | Value::Object(_) => Err(RuntimeError::type_mismatch()),
        }
    }

    /// The numeric kind this operand contributes, or `None` if it is not a
    /// number (arrays; strings are resolved by the caller).
    pub(super) fn num_kind(&self) -> Option<NumKind> {
        match self {
            Value::Empty | Value::Bool(_) | Value::Int(_) => Some(NumKind::Integer),
            Value::Long(_) => Some(NumKind::Long),
            Value::Double(_) | Value::Date(_) => Some(NumKind::Double),
            Value::Str(_) | Value::Null | Value::Array(_) | Value::Object(_) => None,
        }
    }
}

/// Parses a full numeric string (`CDbl` is strict — the whole trimmed string
/// must be a number).
pub(super) fn parse_number(s: &str) -> Result<f64, RuntimeError> {
    s.parse::<f64>().map_err(|_| RuntimeError::type_mismatch())
}

/// Round half to even (banker's rounding), matching VBA's `CLng`/`CInt`.
pub(super) fn round_half_even(x: f64) -> i64 {
    let floor = x.floor();
    let diff = x - floor;
    let rounded = if diff < 0.5 {
        floor
    } else if diff > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    };
    rounded as i64
}

/// Formats a floating value the way BASIC prints it (no trailing zeros, `.`
/// decimal separator). Rust's shortest round-trip formatting matches VBA for
/// the common range.
fn format_number(d: f64) -> String {
    if d == 0.0 {
        return "0".to_string();
    }
    let mut s = format!("{d}");
    if s.contains('e') || s.contains('E') {
        // Leave scientific notation as-is; rare in macro output.
        return s;
    }
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banker_rounding() {
        assert_eq!(round_half_even(2.5), 2);
        assert_eq!(round_half_even(3.5), 4);
        assert_eq!(round_half_even(-2.5), -2);
        assert_eq!(round_half_even(2.4), 2);
        assert_eq!(round_half_even(2.6), 3);
    }

    #[test]
    fn bool_to_number_is_minus_one() {
        assert_eq!(Value::Bool(true).to_f64().unwrap(), -1.0);
        assert_eq!(Value::Bool(false).to_f64().unwrap(), 0.0);
    }

    #[test]
    fn string_number_formatting() {
        assert_eq!(Value::Double(1.5).to_basic_string().unwrap(), "1.5");
        assert_eq!(Value::Double(3.0).to_basic_string().unwrap(), "3");
        assert_eq!(Value::Int(42).to_basic_string().unwrap(), "42");
    }
}
