// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Function dispatch for the formula evaluator: the built-in aggregate table,
//! argument flattening, and the user-defined-function (UDF) entry point (macro
//! spec §6.3). Split from [`super::eval`] for the 300-line ceiling.

use std::collections::HashSet;

use loki_basic::Value;
use loki_sheet_model::Workbook;

use super::eval::Parser;
use super::lexer::Token;
use super::{CellValue, FormulaError, UdfResolver};

/// A function argument: either a scalar expression or an expanded range of the
/// numeric cell values it covers.
pub(super) enum Arg {
    Scalar(f64),
    Range(Vec<f64>),
}

/// Evaluates a formula that is a single UDF call (which may return text). Returns
/// `None` when the formula is not exactly one UDF call — the numeric evaluator
/// then handles it (so a UDF combined with arithmetic is used numerically).
pub(super) fn try_udf_value_formula(
    tokens: &[Token],
    wb: &Workbook,
    visited: &mut HashSet<(usize, usize)>,
    udf: Option<&UdfResolver>,
) -> Result<Option<CellValue>, FormulaError> {
    let Some(resolver) = udf else {
        return Ok(None);
    };
    let name = match tokens.first() {
        Some(Token::Ident(n)) => n.clone(),
        _ => return Ok(None),
    };
    if tokens.get(1) != Some(&Token::LParen) || is_builtin(&name) || !resolver.defines(&name) {
        return Ok(None);
    }
    let mut p = Parser {
        tokens: tokens.to_vec(),
        pos: 2,
        wb,
        visited,
        udf,
    };
    let args = p.collect_args()?;
    p.expect(Token::RParen)?;
    if p.pos != p.tokens.len() {
        return Ok(None); // arithmetic follows → numeric path
    }
    Ok(Some(p.call_udf(&name, &args)?))
}

/// Flattens arguments (scalars and ranges) to a numeric list for aggregates.
pub(super) fn flatten(args: &[Arg]) -> Vec<f64> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Scalar(v) => out.push(*v),
            Arg::Range(vs) => out.extend(vs.iter().copied()),
        }
    }
    out
}

/// Whether `name` is one of the built-in functions (so it is not a UDF).
pub(super) fn is_builtin(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "IF" | "SUM" | "COUNT" | "AVERAGE" | "MIN" | "MAX"
    )
}

/// Maps a UDF's [`Value`] result to a cell value. Numbers/dates are numeric;
/// booleans display as `TRUE`/`FALSE`; strings pass through; blanks are empty;
/// arrays/objects have no cell representation (`#VALUE!`).
pub(super) fn value_to_cell(v: Value) -> Result<CellValue, FormulaError> {
    match v {
        Value::Int(_) | Value::Long(_) | Value::Double(_) | Value::Date(_) => {
            Ok(CellValue::Num(v.to_f64().map_err(|_| FormulaError::Value)?))
        }
        Value::Bool(b) => Ok(CellValue::Text(
            if b { "TRUE" } else { "FALSE" }.to_string(),
        )),
        Value::Str(s) => Ok(CellValue::Text(s)),
        Value::Empty | Value::Null => Ok(CellValue::Text(String::new())),
        Value::Array(_) | Value::Object(_) => Err(FormulaError::Value),
    }
}

/// Dispatches a built-in aggregate function by name.
pub(super) fn dispatch(name: &str, args: &[Arg]) -> Result<f64, FormulaError> {
    match name.to_ascii_uppercase().as_str() {
        "SUM" => Ok(flatten(args).iter().sum()),
        "COUNT" => Ok(flatten(args).len() as f64),
        "AVERAGE" => {
            let vals = flatten(args);
            if vals.is_empty() {
                Err(FormulaError::Div0)
            } else {
                Ok(vals.iter().sum::<f64>() / vals.len() as f64)
            }
        }
        "MIN" => {
            let v = flatten(args).into_iter().fold(f64::INFINITY, f64::min);
            Ok(if v.is_finite() { v } else { 0.0 })
        }
        "MAX" => {
            let v = flatten(args).into_iter().fold(f64::NEG_INFINITY, f64::max);
            Ok(if v.is_finite() { v } else { 0.0 })
        }
        // `IF` is handled lazily in `parse_if_function` before reaching here.
        _ => Err(FormulaError::Name),
    }
}
