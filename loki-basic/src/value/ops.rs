// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Operator semantics over [`Value`]: arithmetic, concatenation, and the
//! [`binary_op`]/[`unary_op`] dispatch. Comparison, logical, and `Like` live in
//! [`super::ops_logic`].

use super::Value;
use super::coerce::NumKind;
use super::ops_logic;
use crate::ast::{BinOp, UnOp};
use crate::error::RuntimeError;

/// Applies a binary operator, following VBA/`StarBasic` coercion rules.
///
/// `compare_text` selects case-insensitive string comparison (`Option Compare
/// Text`).
///
/// # Errors
///
/// Propagates coercion errors ([`RuntimeError::type_mismatch`]), division by
/// zero, and overflow.
pub fn binary_op(
    op: BinOp,
    lhs: &Value,
    rhs: &Value,
    compare_text: bool,
) -> Result<Value, RuntimeError> {
    // `Is` compares object identity (and `Nothing`), so it must run *before* the
    // Null short-circuit — `obj Is Nothing` returns a Boolean, not Null.
    if op == BinOp::Is {
        return ops_logic::is_identity(lhs, rhs);
    }
    // `&` treats Null as ""; everything else propagates Null.
    if op != BinOp::Concat && (lhs.is_null() || rhs.is_null()) {
        return Ok(Value::Null);
    }
    match op {
        BinOp::Add => add(lhs, rhs),
        BinOp::Sub => arith(lhs, rhs, Arith::Sub),
        BinOp::Mul => arith(lhs, rhs, Arith::Mul),
        BinOp::Div => divide(lhs, rhs),
        BinOp::IntDiv => int_div(lhs, rhs),
        BinOp::Mod => modulo(lhs, rhs),
        BinOp::Pow => Ok(Value::Double(lhs.to_f64()?.powf(rhs.to_f64()?))),
        BinOp::Concat => concat(lhs, rhs),
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            ops_logic::compare(op, lhs, rhs, compare_text)
        }
        BinOp::And | BinOp::Or | BinOp::Xor | BinOp::Eqv | BinOp::Imp => {
            ops_logic::logical(op, lhs, rhs)
        }
        BinOp::Like => ops_logic::like(lhs, rhs, compare_text),
        // Handled above, before the Null short-circuit.
        BinOp::Is => ops_logic::is_identity(lhs, rhs),
    }
}

/// Applies a unary operator.
///
/// # Errors
///
/// Propagates coercion errors.
pub fn unary_op(op: UnOp, v: &Value) -> Result<Value, RuntimeError> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    match op {
        UnOp::Pos => {
            // Unary plus yields the numeric value unchanged.
            if v.num_kind().is_some() {
                Ok(v.clone())
            } else {
                Ok(Value::Double(v.to_f64()?))
            }
        }
        UnOp::Neg => negate(v),
        UnOp::Not => ops_logic::not(v),
    }
}

#[derive(Clone, Copy)]
enum Arith {
    Sub,
    Mul,
}

/// The `+` operator: string concatenation for two strings, numeric addition
/// otherwise (strings coerced; type mismatch on a non-numeric string).
fn add(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    if matches!(lhs, Value::Str(_)) && matches!(rhs, Value::Str(_)) {
        return concat(lhs, rhs);
    }
    match result_kind(lhs, rhs) {
        NumKind::Double => Ok(Value::Double(lhs.to_f64()? + rhs.to_f64()?)),
        kind => {
            let r = lhs
                .to_i64_round()?
                .checked_add(rhs.to_i64_round()?)
                .ok_or_else(RuntimeError::overflow)?;
            narrow(r, kind)
        }
    }
}

fn arith(lhs: &Value, rhs: &Value, op: Arith) -> Result<Value, RuntimeError> {
    match result_kind(lhs, rhs) {
        NumKind::Double => {
            let (a, b) = (lhs.to_f64()?, rhs.to_f64()?);
            Ok(Value::Double(match op {
                Arith::Sub => a - b,
                Arith::Mul => a * b,
            }))
        }
        kind => {
            let (a, b) = (lhs.to_i64_round()?, rhs.to_i64_round()?);
            let r = match op {
                Arith::Sub => a.checked_sub(b),
                Arith::Mul => a.checked_mul(b),
            }
            .ok_or_else(RuntimeError::overflow)?;
            narrow(r, kind)
        }
    }
}

fn divide(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    let b = rhs.to_f64()?;
    if b == 0.0 {
        return Err(RuntimeError::division_by_zero());
    }
    Ok(Value::Double(lhs.to_f64()? / b))
}

fn int_div(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    let (a, b) = (lhs.to_i64_round()?, rhs.to_i64_round()?);
    if b == 0 {
        return Err(RuntimeError::division_by_zero());
    }
    let q = a.checked_div(b).ok_or_else(RuntimeError::overflow)?;
    Ok(Value::from_i64_fit(q))
}

fn modulo(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    let (a, b) = (lhs.to_i64_round()?, rhs.to_i64_round()?);
    if b == 0 {
        return Err(RuntimeError::division_by_zero());
    }
    let r = a.checked_rem(b).ok_or_else(RuntimeError::overflow)?;
    Ok(Value::from_i64_fit(r))
}

fn negate(v: &Value) -> Result<Value, RuntimeError> {
    match v.num_kind() {
        Some(NumKind::Double) | None => Ok(Value::Double(-v.to_f64()?)),
        Some(kind) => {
            let r = v
                .to_i64_round()?
                .checked_neg()
                .ok_or_else(RuntimeError::overflow)?;
            narrow(r, kind)
        }
    }
}

/// String concatenation (`&`): `Null` operands act as `""` unless both are
/// `Null` (then the result is `Null`).
fn concat(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    if lhs.is_null() && rhs.is_null() {
        return Ok(Value::Null);
    }
    let a = if lhs.is_null() {
        String::new()
    } else {
        lhs.to_basic_string()?
    };
    let b = if rhs.is_null() {
        String::new()
    } else {
        rhs.to_basic_string()?
    };
    Ok(Value::Str(a + &b))
}

/// The arithmetic result kind: the wider of the two operands' kinds, with
/// non-numeric operands (strings) treated as `Double`.
fn result_kind(lhs: &Value, rhs: &Value) -> NumKind {
    let a = lhs.num_kind().unwrap_or(NumKind::Double);
    let b = rhs.num_kind().unwrap_or(NumKind::Double);
    a.max(b)
}

/// Narrows an integer result to the target kind's storage type, erroring on
/// overflow (VBA does not auto-widen an overflowing `Integer`/`Long`).
fn narrow(n: i64, kind: NumKind) -> Result<Value, RuntimeError> {
    match kind {
        NumKind::Integer => i16::try_from(n)
            .map(Value::Int)
            .map_err(|_| RuntimeError::overflow()),
        NumKind::Long => i32::try_from(n)
            .map(Value::Long)
            .map_err(|_| RuntimeError::overflow()),
        NumKind::Double => Ok(Value::Double(n as f64)),
    }
}
