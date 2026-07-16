// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! String built-ins. Positions are 1-based and operate on Unicode scalar
//! values (a documented approximation of VBA's UTF-16 code-unit positions —
//! identical for the BMP/ASCII text that dominates macros).

use super::{arg, require};
use crate::error::RuntimeError;
use crate::value::Value;

pub(super) fn call(name: &str, a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 1)?;
    let s = |i: usize| arg(a, i).to_basic_string();
    match name {
        "len" => Ok(Value::Long(s(0)?.chars().count() as i32)),
        "ucase" => Ok(Value::Str(s(0)?.to_uppercase())),
        "lcase" => Ok(Value::Str(s(0)?.to_lowercase())),
        "trim" => Ok(Value::Str(s(0)?.trim().to_string())),
        "ltrim" => Ok(Value::Str(s(0)?.trim_start().to_string())),
        "rtrim" => Ok(Value::Str(s(0)?.trim_end().to_string())),
        "strreverse" => Ok(Value::Str(s(0)?.chars().rev().collect())),
        "left" => left_right(&s(0)?, arg(a, 1).to_i32()?, true),
        "right" => left_right(&s(0)?, arg(a, 1).to_i32()?, false),
        "mid" => mid(a),
        "space" => repeat(' ', arg(a, 0).to_i32()?),
        "string" => string_fn(a),
        "chr" => chr(arg(a, 0).to_i32()?),
        "asc" => asc(&s(0)?),
        "instr" => instr(a),
        "instrrev" => instrrev(a),
        "replace" => replace(a),
        _ => Err(RuntimeError::invalid_call()),
    }
}

fn left_right(s: &str, n: i32, left: bool) -> Result<Value, RuntimeError> {
    if n < 0 {
        return Err(RuntimeError::invalid_call());
    }
    let n = n as usize;
    let chars: Vec<char> = s.chars().collect();
    let out: String = if left {
        chars.iter().take(n).collect()
    } else {
        let start = chars.len().saturating_sub(n);
        chars[start..].iter().collect()
    };
    Ok(Value::Str(out))
}

fn mid(a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 2)?;
    let s: Vec<char> = arg(a, 0).to_basic_string()?.chars().collect();
    let start = arg(a, 1).to_i32()?;
    if start < 1 {
        return Err(RuntimeError::invalid_call());
    }
    let start = (start - 1) as usize;
    let take = if a.len() >= 3 {
        arg(a, 2).to_i32()?.max(0) as usize
    } else {
        s.len()
    };
    let out: String = s.iter().skip(start).take(take).collect();
    Ok(Value::Str(out))
}

fn repeat(c: char, n: i32) -> Result<Value, RuntimeError> {
    if n < 0 {
        return Err(RuntimeError::invalid_call());
    }
    Ok(Value::Str(std::iter::repeat_n(c, n as usize).collect()))
}

/// `Space(n)` reaches here via `repeat`; `String(n, char)`:
fn string_fn(a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 2)?;
    let n = arg(a, 0).to_i32()?;
    if n < 0 {
        return Err(RuntimeError::invalid_call());
    }
    let ch = match arg(a, 1) {
        Value::Str(s) => s.chars().next().unwrap_or(' '),
        other => char::from_u32(u32::try_from(other.to_i32()?).unwrap_or(32)).unwrap_or(' '),
    };
    Ok(Value::Str(std::iter::repeat_n(ch, n as usize).collect()))
}

fn chr(code: i32) -> Result<Value, RuntimeError> {
    let c = u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(RuntimeError::invalid_call)?;
    Ok(Value::Str(c.to_string()))
}

fn asc(s: &str) -> Result<Value, RuntimeError> {
    s.chars()
        .next()
        .map(|c| Value::Long(c as i32))
        .ok_or_else(RuntimeError::invalid_call)
}

/// `InStr([start,] haystack, needle)` — 1-based position or 0.
fn instr(a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 2)?;
    let (start, hay, needle) =
        if a.len() >= 3 && matches!(arg(a, 0), Value::Int(_) | Value::Long(_) | Value::Double(_)) {
            (arg(a, 0).to_i32()?.max(1), arg(a, 1), arg(a, 2))
        } else {
            (1, arg(a, 0), arg(a, 1))
        };
    let hay: Vec<char> = hay.to_basic_string()?.chars().collect();
    let needle: Vec<char> = needle.to_basic_string()?.chars().collect();
    let from = (start - 1) as usize;
    Ok(Value::Long(
        find(&hay, &needle, from).map_or(0, |p| p as i32 + 1),
    ))
}

fn instrrev(a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 2)?;
    let hay: Vec<char> = arg(a, 0).to_basic_string()?.chars().collect();
    let needle: Vec<char> = arg(a, 1).to_basic_string()?.chars().collect();
    if needle.is_empty() {
        return Ok(Value::Long(0));
    }
    let mut best = 0;
    let mut from = 0;
    while let Some(p) = find(&hay, &needle, from) {
        best = p as i32 + 1;
        from = p + 1;
    }
    Ok(Value::Long(best))
}

fn replace(a: &[Value]) -> Result<Value, RuntimeError> {
    require(a, 3)?;
    let src = arg(a, 0).to_basic_string()?;
    let find = arg(a, 1).to_basic_string()?;
    let repl = arg(a, 2).to_basic_string()?;
    if find.is_empty() {
        return Ok(Value::Str(src));
    }
    Ok(Value::Str(src.replace(&find, &repl)))
}

fn find(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(hay.len()));
    }
    if from >= hay.len() {
        return None;
    }
    (from..=hay.len().saturating_sub(needle.len())).find(|&i| hay[i..i + needle.len()] == *needle)
}
