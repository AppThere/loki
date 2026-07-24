// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Value-model operator tests via the public `binary_op` / `unary_op` API.

use loki_basic::Value;
use loki_basic::ast::{BinOp, UnOp};
use loki_basic::value::{binary_op, unary_op};

fn bin(op: BinOp, a: Value, b: Value) -> Value {
    binary_op(op, &a, &b, false).expect("binary_op")
}

#[test]
fn integer_addition_stays_integer() {
    assert_eq!(bin(BinOp::Add, Value::Int(2), Value::Int(3)), Value::Int(5));
}

#[test]
fn integer_overflow_errors_not_wraps() {
    let r = binary_op(BinOp::Add, &Value::Int(30000), &Value::Int(30000), false);
    assert!(r.is_err(), "Integer+Integer overflow must raise error 6");
}

#[test]
fn integer_plus_long_widens_to_long() {
    assert_eq!(
        bin(BinOp::Add, Value::Int(1), Value::Long(70000)),
        Value::Long(70001)
    );
}

#[test]
fn division_is_always_double() {
    assert_eq!(
        bin(BinOp::Div, Value::Int(7), Value::Int(2)),
        Value::Double(3.5)
    );
}

#[test]
fn division_by_zero_errors() {
    assert!(binary_op(BinOp::Div, &Value::Int(1), &Value::Int(0), false).is_err());
}

#[test]
fn integer_division_truncates() {
    assert_eq!(
        bin(BinOp::IntDiv, Value::Int(7), Value::Int(2)),
        Value::Int(3)
    );
    assert_eq!(
        bin(BinOp::IntDiv, Value::Int(-7), Value::Int(2)),
        Value::Int(-3)
    );
}

#[test]
fn modulo_takes_dividend_sign() {
    assert_eq!(
        bin(BinOp::Mod, Value::Int(-7), Value::Int(3)),
        Value::Int(-1)
    );
}

#[test]
fn pow_is_double() {
    assert_eq!(
        bin(BinOp::Pow, Value::Int(2), Value::Int(10)),
        Value::Double(1024.0)
    );
}

#[test]
fn concat_coerces_and_joins() {
    assert_eq!(
        bin(BinOp::Concat, Value::Str("x=".into()), Value::Int(5)),
        Value::Str("x=5".into())
    );
}

#[test]
fn concat_treats_null_as_empty() {
    assert_eq!(
        bin(BinOp::Concat, Value::Null, Value::Str("a".into())),
        Value::Str("a".into())
    );
    assert_eq!(bin(BinOp::Concat, Value::Null, Value::Null), Value::Null);
}

#[test]
fn null_propagates_through_arithmetic() {
    assert_eq!(bin(BinOp::Add, Value::Null, Value::Int(1)), Value::Null);
}

#[test]
fn string_plus_string_concatenates() {
    assert_eq!(
        bin(BinOp::Add, Value::Str("a".into()), Value::Str("b".into())),
        Value::Str("ab".into())
    );
}

#[test]
fn comparison_returns_boolean() {
    assert_eq!(
        bin(BinOp::Lt, Value::Int(1), Value::Int(2)),
        Value::Bool(true)
    );
    assert_eq!(
        bin(BinOp::Eq, Value::Int(2), Value::Int(2)),
        Value::Bool(true)
    );
}

#[test]
fn text_comparison_is_case_insensitive() {
    let a = Value::Str("ABC".into());
    let b = Value::Str("abc".into());
    assert_eq!(
        binary_op(BinOp::Eq, &a, &b, true).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        binary_op(BinOp::Eq, &a, &b, false).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn boolean_logic_stays_boolean() {
    assert_eq!(
        bin(BinOp::And, Value::Bool(true), Value::Bool(false)),
        Value::Bool(false)
    );
    assert_eq!(
        unary_op(UnOp::Not, &Value::Bool(true)).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn bitwise_and_on_integers() {
    assert_eq!(bin(BinOp::And, Value::Int(6), Value::Int(3)), Value::Int(2));
}

#[test]
fn negate_preserves_kind() {
    assert_eq!(unary_op(UnOp::Neg, &Value::Int(5)).unwrap(), Value::Int(-5));
    assert_eq!(
        unary_op(UnOp::Neg, &Value::Double(2.5)).unwrap(),
        Value::Double(-2.5)
    );
}

#[test]
fn like_patterns() {
    let hay = Value::Str("hello".into());
    assert_eq!(
        bin(BinOp::Like, hay.clone(), Value::Str("h*o".into())),
        Value::Bool(true)
    );
    assert_eq!(
        bin(BinOp::Like, hay.clone(), Value::Str("h?llo".into())),
        Value::Bool(true)
    );
    assert_eq!(
        bin(BinOp::Like, hay.clone(), Value::Str("[a-h]ello".into())),
        Value::Bool(true)
    );
    assert_eq!(
        bin(BinOp::Like, hay, Value::Str("x*".into())),
        Value::Bool(false)
    );
    assert_eq!(
        bin(
            BinOp::Like,
            Value::Str("a1b".into()),
            Value::Str("a#b".into())
        ),
        Value::Bool(true)
    );
}

#[test]
fn empty_acts_as_zero_in_arithmetic() {
    assert_eq!(bin(BinOp::Add, Value::Empty, Value::Int(5)), Value::Int(5));
}
