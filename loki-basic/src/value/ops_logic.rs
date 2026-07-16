// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Comparison, logical, and `Like` operators over [`Value`]. Split from
//! [`super::ops`] for the 300-line ceiling.

use std::cmp::Ordering;

use super::Value;
use crate::ast::BinOp;
use crate::error::RuntimeError;

/// Comparison operators (`= <> < <= > >=`) returning a `Boolean`.
pub(super) fn compare(
    op: BinOp,
    lhs: &Value,
    rhs: &Value,
    compare_text: bool,
) -> Result<Value, RuntimeError> {
    let ord = ordering(lhs, rhs, compare_text)?;
    let r = match op {
        BinOp::Eq => ord == Ordering::Equal,
        BinOp::Ne => ord != Ordering::Equal,
        BinOp::Lt => ord == Ordering::Less,
        BinOp::Le => ord != Ordering::Greater,
        BinOp::Gt => ord == Ordering::Greater,
        BinOp::Ge => ord != Ordering::Less,
        _ => unreachable!("compare called with non-comparison op"),
    };
    Ok(Value::Bool(r))
}

fn ordering(lhs: &Value, rhs: &Value, compare_text: bool) -> Result<Ordering, RuntimeError> {
    if let (Value::Str(a), Value::Str(b)) = (lhs, rhs) {
        return Ok(string_cmp(a, b, compare_text));
    }
    let (a, b) = (lhs.to_f64()?, rhs.to_f64()?);
    a.partial_cmp(&b).ok_or_else(RuntimeError::type_mismatch)
}

fn string_cmp(a: &str, b: &str, compare_text: bool) -> Ordering {
    if compare_text {
        a.to_lowercase().cmp(&b.to_lowercase())
    } else {
        a.cmp(b)
    }
}

/// Logical/bitwise operators (`And Or Xor Eqv Imp`). Two booleans give a
/// boolean; otherwise the operands are coerced to integers and the operator is
/// bitwise.
pub(super) fn logical(op: BinOp, lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    if let (Value::Bool(a), Value::Bool(b)) = (lhs, rhs) {
        let r = match op {
            BinOp::And => *a && *b,
            BinOp::Or => *a || *b,
            BinOp::Xor => *a ^ *b,
            BinOp::Eqv => *a == *b,
            BinOp::Imp => !*a || *b,
            _ => unreachable!("logical called with non-logical op"),
        };
        return Ok(Value::Bool(r));
    }
    let (a, b) = (lhs.to_i64_round()?, rhs.to_i64_round()?);
    let r = match op {
        BinOp::And => a & b,
        BinOp::Or => a | b,
        BinOp::Xor => a ^ b,
        BinOp::Eqv => !(a ^ b),
        BinOp::Imp => (!a) | b,
        _ => unreachable!("logical called with non-logical op"),
    };
    Ok(Value::from_i64_fit(r))
}

/// The `Not` unary operator: boolean negation, else bitwise complement.
pub(super) fn not(v: &Value) -> Result<Value, RuntimeError> {
    if let Value::Bool(b) = v {
        return Ok(Value::Bool(!b));
    }
    Ok(Value::from_i64_fit(!v.to_i64_round()?))
}

/// The `Like` pattern operator (`?` any char, `*` any run, `#` any digit,
/// `[…]`/`[!…]` char class with ranges).
pub(super) fn like(lhs: &Value, rhs: &Value, compare_text: bool) -> Result<Value, RuntimeError> {
    let text: Vec<char> = lhs.to_basic_string()?.chars().collect();
    let pat: Vec<char> = rhs.to_basic_string()?.chars().collect();
    Ok(Value::Bool(like_match(&text, &pat, compare_text)))
}

fn like_match(text: &[char], pat: &[char], ci: bool) -> bool {
    let Some(&p0) = pat.first() else {
        return text.is_empty();
    };
    match p0 {
        '*' => {
            like_match(text, &pat[1..], ci) || (!text.is_empty() && like_match(&text[1..], pat, ci))
        }
        '?' => !text.is_empty() && like_match(&text[1..], &pat[1..], ci),
        '#' => {
            !text.is_empty() && text[0].is_ascii_digit() && like_match(&text[1..], &pat[1..], ci)
        }
        '[' => match match_class(text.first().copied(), pat, ci) {
            Some((true, rest)) if !text.is_empty() => like_match(&text[1..], rest, ci),
            Some((_, _)) => false,
            None => {
                // Malformed class: treat `[` literally.
                !text.is_empty()
                    && char_eq(text[0], '[', ci)
                    && like_match(&text[1..], &pat[1..], ci)
            }
        },
        c => !text.is_empty() && char_eq(text[0], c, ci) && like_match(&text[1..], &pat[1..], ci),
    }
}

/// Matches `ch` against a `[…]` class beginning at `pat[0] == '['`. Returns
/// `(matched, pattern_after_close)` or `None` if the class is unterminated.
fn match_class(ch: Option<char>, pat: &[char], ci: bool) -> Option<(bool, &[char])> {
    let mut i = 1; // past '['
    let negate = pat.get(i) == Some(&'!');
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut has_content = false;
    while i < pat.len() && pat[i] != ']' {
        has_content = true;
        // Range `a-z`.
        if i + 2 < pat.len() && pat[i + 1] == '-' && pat[i + 2] != ']' {
            if let Some(c) = ch {
                let (lo, hi) = (pat[i], pat[i + 2]);
                if in_range(c, lo, hi, ci) {
                    matched = true;
                }
            }
            i += 3;
        } else {
            if let Some(c) = ch
                && char_eq(c, pat[i], ci)
            {
                matched = true;
            }
            i += 1;
        }
    }
    if i >= pat.len() || !has_content {
        return None; // unterminated or empty `[]`
    }
    let rest = &pat[i + 1..]; // past ']'
    Some((matched ^ negate, rest))
}

fn in_range(c: char, lo: char, hi: char, ci: bool) -> bool {
    if ci {
        let c = c.to_ascii_lowercase();
        c >= lo.to_ascii_lowercase() && c <= hi.to_ascii_lowercase()
    } else {
        c >= lo && c <= hi
    }
}

fn char_eq(a: char, b: char, ci: bool) -> bool {
    if ci {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}
