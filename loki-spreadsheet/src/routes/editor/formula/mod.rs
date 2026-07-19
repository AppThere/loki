// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spreadsheet formula evaluation.
//!
//! A small recursive-descent evaluator over the [`Workbook`] model supporting
//! arithmetic with operator precedence and parentheses (`+ - * /`), A1 cell
//! references and ranges, the aggregate functions `SUM`, `AVERAGE`, `MIN`,
//! `MAX`, `COUNT`, and `IF(cond, a, b)`. Unresolvable input yields an Excel-style
//! error value (`#NAME?`, `#VALUE!`, `#DIV/0!`, `#REF!`, `#NUM!`) rather than a
//! silent `0`.
//!
//! Cell references resolve through [`evaluate_cell`], which guards against
//! reference cycles. Non-numeric and empty cells are treated as `0` in
//! arithmetic and excluded from aggregates (matching Excel's blank handling).

mod eval;
mod funcs;
mod lexer;
pub(crate) mod udf;

use std::collections::HashSet;

use loki_sheet_model::{CellStyle, NumberFormat, Workbook};

pub(crate) use udf::UdfResolver;

/// A computed formula value — a number, or text (a text-returning UDF, §6.3).
pub(crate) enum CellValue {
    /// A numeric result (the only kind the arithmetic evaluator produces).
    Num(f64),
    /// A text result — only a whole-formula UDF call can yield this.
    Text(String),
}

/// An Excel-style formula error value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormulaError {
    /// Unknown function or unparseable reference.
    Name,
    /// Malformed expression / wrong argument shape.
    Value,
    /// Division by zero (or average of no values).
    Div0,
    /// Reference cycle or otherwise invalid reference.
    Ref,
    /// Non-finite numeric result.
    Num,
    /// A user-defined function failed, was refused, exhausted its budget, or
    /// tried to reach outside its compute-only sandbox (macro spec §6.3).
    Macro,
}

impl FormulaError {
    /// The cell-displayed error code.
    pub(crate) fn code(self) -> &'static str {
        match self {
            FormulaError::Name => "#NAME?",
            FormulaError::Value => "#VALUE!",
            FormulaError::Div0 => "#DIV/0!",
            FormulaError::Ref => "#REF!",
            FormulaError::Num => "#NUM!",
            FormulaError::Macro => "#MACRO!",
        }
    }

    /// Maps a referenced cell's displayed error code back to a [`FormulaError`]
    /// so it propagates through the calling formula (Excel error contagion).
    fn from_code(s: &str) -> Option<Self> {
        match s {
            "#NAME?" => Some(FormulaError::Name),
            "#VALUE!" => Some(FormulaError::Value),
            "#DIV/0!" => Some(FormulaError::Div0),
            "#REF!" => Some(FormulaError::Ref),
            "#NUM!" => Some(FormulaError::Num),
            "#MACRO!" => Some(FormulaError::Macro),
            _ => None,
        }
    }
}

/// Evaluates a cell to the string it should display.
///
/// Returns the raw value for a plain cell, the formatted numeric result for a
/// formula cell, or an error code. `visited` tracks the active evaluation chain
/// so a reference cycle resolves to `#REF!` instead of recursing forever.
pub(crate) fn evaluate_cell(
    row: usize,
    col: usize,
    wb: &Workbook,
    visited: &mut HashSet<(usize, usize)>,
    udf: Option<&UdfResolver>,
) -> String {
    if !visited.insert((row, col)) {
        return "#REF!".to_string();
    }
    let result = eval_cell_inner(row, col, wb, visited, udf);
    visited.remove(&(row, col));
    result
}

fn eval_cell_inner(
    row: usize,
    col: usize,
    wb: &Workbook,
    visited: &mut HashSet<(usize, usize)>,
    udf: Option<&UdfResolver>,
) -> String {
    let Some(sheet) = wb.get_sheet(0) else {
        return String::new();
    };
    let Some(cell) = sheet.get_cell(row as u32, col as u32) else {
        return String::new();
    };
    match &cell.formula {
        None => cell.value.clone(),
        // `cell.formula` is populated only when the cell holds a formula (the
        // editor strips the leading `=` before storing, and importers store the
        // bare expression), so a present formula is always evaluated.
        Some(f) => match evaluate_formula(f, wb, visited, udf) {
            Ok(CellValue::Num(v)) => format_number(v),
            Ok(CellValue::Text(s)) => s,
            Err(e) => e.code().to_string(),
        },
    }
}

/// Evaluates a formula expression (with or without a leading `=`) to a value.
///
/// `udf` supplies the workbook's user-defined functions; when `None`, an unknown
/// function name is a plain `#NAME?` (no macro payload / no UDFs).
pub(crate) fn evaluate_formula(
    formula: &str,
    wb: &Workbook,
    visited: &mut HashSet<(usize, usize)>,
    udf: Option<&UdfResolver>,
) -> Result<CellValue, FormulaError> {
    let expr = formula.trim().strip_prefix('=').unwrap_or(formula.trim());
    if expr.is_empty() {
        return Ok(CellValue::Num(0.0));
    }
    let tokens = lexer::tokenize(expr)?;
    if tokens.is_empty() {
        return Ok(CellValue::Num(0.0));
    }
    // A whole-formula UDF call may return text; anything else is numeric.
    if let Some(cv) = funcs::try_udf_value_formula(&tokens, wb, visited, udf)? {
        return Ok(cv);
    }
    let value = eval::evaluate_tokens(tokens, wb, visited, udf)?;
    if !value.is_finite() {
        return Err(FormulaError::Num);
    }
    Ok(CellValue::Num(value))
}

/// Applies the cell's number format to an already-evaluated string value.
pub(crate) fn format_evaluated_value(val_str: &str, format: &CellStyle) -> String {
    if let Ok(num) = val_str.parse::<f64>() {
        match format.num_format {
            NumberFormat::Currency => format!("${num:.2}"),
            NumberFormat::Percent => format!("{:.1}%", num * 100.0),
            NumberFormat::General => val_str.to_string(),
        }
    } else {
        val_str.to_string()
    }
}

/// Renders an evaluated number, suppressing floating-point noise and trailing
/// zeros (`3.0` → `"3"`, `0.1 + 0.2` → `"0.3"`).
fn format_number(v: f64) -> String {
    let rounded = (v * 1e10).round() / 1e10;
    if rounded == rounded.trunc() && rounded.abs() < 1e15 {
        return format!("{}", rounded as i64);
    }
    let mut s = format!("{rounded:.10}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}
