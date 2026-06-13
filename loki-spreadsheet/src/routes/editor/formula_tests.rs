// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the spreadsheet formula evaluator.

use std::collections::HashSet;

use loki_sheet_model::{CellAlign, CellStyle, NumberFormat, Workbook};

use super::formula::{FormulaError, evaluate_cell, evaluate_formula, format_evaluated_value};

/// Build a single-sheet workbook from `(row, col, value, formula)` tuples.
fn wb(cells: &[(u32, u32, &str, Option<&str>)]) -> Workbook {
    let mut wb = Workbook::new();
    let sheet = wb.get_sheet_mut(0).unwrap();
    for &(r, c, value, formula) in cells {
        let cell = sheet.get_cell_mut(r, c);
        cell.value = value.to_string();
        cell.formula = formula.map(str::to_string);
    }
    wb
}

/// Evaluate a bare expression against a workbook.
fn eval(expr: &str, workbook: &Workbook) -> Result<f64, FormulaError> {
    let mut visited = HashSet::new();
    evaluate_formula(expr, workbook, &mut visited)
}

/// Evaluate a cell to its displayed string.
fn display(row: usize, col: usize, workbook: &Workbook) -> String {
    let mut visited = HashSet::new();
    evaluate_cell(row, col, workbook, &mut visited)
}

// ── Arithmetic & precedence ─────────────────────────────────────────────────────

#[test]
fn arithmetic_precedence_and_parens() {
    let w = Workbook::new();
    assert_eq!(eval("1+2*3", &w), Ok(7.0));
    assert_eq!(eval("(1+2)*3", &w), Ok(9.0));
    assert_eq!(eval("2*3+4*5", &w), Ok(26.0));
    assert_eq!(eval("10-3-2", &w), Ok(5.0)); // left-associative
    assert_eq!(eval("2*(3+(4-1))", &w), Ok(12.0));
}

#[test]
fn division_and_div_by_zero() {
    let w = Workbook::new();
    assert_eq!(eval("10/4", &w), Ok(2.5));
    assert_eq!(eval("1/0", &w), Err(FormulaError::Div0));
    assert_eq!(eval("8/2/2", &w), Ok(2.0));
}

#[test]
fn unary_minus_and_plus() {
    let w = Workbook::new();
    assert_eq!(eval("-5+3", &w), Ok(-2.0));
    assert_eq!(eval("-(2+3)", &w), Ok(-5.0));
    assert_eq!(eval("+4", &w), Ok(4.0));
    assert_eq!(eval("3*-2", &w), Ok(-6.0));
}

#[test]
fn floating_point_noise_is_trimmed_in_display() {
    // 0.1 + 0.2 must display as "0.3", not 0.30000000000000004.
    let w = wb(&[(0, 0, "", Some("0.1+0.2"))]);
    assert_eq!(display(0, 0, &w), "0.3");
    // Integer-valued results render without a decimal point.
    let w2 = wb(&[(0, 0, "", Some("2*3"))]);
    assert_eq!(display(0, 0, &w2), "6");
}

// ── Cell references ─────────────────────────────────────────────────────────────

#[test]
fn references_resolve_and_combine() {
    let w = wb(&[(0, 0, "10", None), (0, 1, "5", None)]);
    assert_eq!(eval("A1+B1", &w), Ok(15.0));
    assert_eq!(eval("A1*B1", &w), Ok(50.0));
    assert_eq!(eval("A1/B1", &w), Ok(2.0));
}

#[test]
fn empty_and_text_cells_count_as_zero_in_arithmetic() {
    let w = wb(&[(0, 0, "hello", None)]); // text
    assert_eq!(eval("A1+5", &w), Ok(5.0));
    assert_eq!(eval("B9+5", &w), Ok(5.0)); // empty cell
}

#[test]
fn multi_letter_column_reference() {
    let w = wb(&[(0, 26, "7", None)]); // AA1
    assert_eq!(eval("AA1*2", &w), Ok(14.0));
}

// ── Aggregate functions ─────────────────────────────────────────────────────────

#[test]
fn sum_over_range_and_mixed_args() {
    let w = wb(&[
        (0, 0, "1", None),
        (1, 0, "2", None),
        (2, 0, "3", None),
        (0, 1, "10", None),
    ]);
    assert_eq!(eval("SUM(A1:A3)", &w), Ok(6.0));
    assert_eq!(eval("SUM(A1:A3, B1, 100)", &w), Ok(116.0));
    assert_eq!(eval("SUM(A1:A3)*2", &w), Ok(12.0));
}

#[test]
fn average_min_max_count() {
    let w = wb(&[
        (0, 0, "2", None),
        (1, 0, "4", None),
        (2, 0, "9", None),
        // (3,0) intentionally empty
        (4, 0, "text", None),
    ]);
    assert_eq!(eval("AVERAGE(A1:A3)", &w), Ok(5.0));
    assert_eq!(eval("MIN(A1:A5)", &w), Ok(2.0));
    assert_eq!(eval("MAX(A1:A5)", &w), Ok(9.0));
    // COUNT ignores the empty and text cells.
    assert_eq!(eval("COUNT(A1:A5)", &w), Ok(3.0));
}

#[test]
fn average_of_no_values_is_div0() {
    let w = Workbook::new();
    assert_eq!(eval("AVERAGE(B1:B5)", &w), Err(FormulaError::Div0));
}

#[test]
fn min_max_of_empty_range_is_zero() {
    let w = Workbook::new();
    assert_eq!(eval("MIN(B1:B5)", &w), Ok(0.0));
    assert_eq!(eval("MAX(B1:B5)", &w), Ok(0.0));
}

#[test]
fn function_names_are_case_insensitive() {
    let w = wb(&[(0, 0, "3", None), (1, 0, "4", None)]);
    assert_eq!(eval("sum(a1:a2)", &w), Ok(7.0));
    assert_eq!(eval("Sum(A1:A2)", &w), Ok(7.0));
}

// ── IF ───────────────────────────────────────────────────────────────────────────

#[test]
fn if_selects_branch_on_truthiness() {
    let w = wb(&[(0, 0, "1", None)]);
    assert_eq!(eval("IF(A1, 10, 20)", &w), Ok(10.0));
    assert_eq!(eval("IF(A1-A1, 10, 20)", &w), Ok(20.0)); // 0 → false
    assert_eq!(eval("IF(2*3, 1, 2)", &w), Ok(1.0));
}

#[test]
fn if_wrong_arity_is_value_error() {
    let w = Workbook::new();
    assert_eq!(eval("IF(1, 2)", &w), Err(FormulaError::Value));
}

// ── Errors ───────────────────────────────────────────────────────────────────────

#[test]
fn unknown_function_is_name_error() {
    let w = Workbook::new();
    assert_eq!(eval("FOO(1,2)", &w), Err(FormulaError::Name));
}

#[test]
fn bare_unparseable_identifier_is_name_error() {
    let w = Workbook::new();
    assert_eq!(eval("XYZ", &w), Err(FormulaError::Name));
}

#[test]
fn trailing_tokens_are_value_error() {
    let w = Workbook::new();
    assert_eq!(eval("1 2", &w), Err(FormulaError::Value));
    assert_eq!(eval("1+", &w), Err(FormulaError::Value));
}

#[test]
fn reference_cycle_displays_ref_error() {
    // A1 = B1, B1 = A1 → cycle.
    let w = wb(&[(0, 0, "", Some("B1")), (0, 1, "", Some("A1"))]);
    assert_eq!(display(0, 0, &w), "#REF!");
    assert_eq!(display(0, 1, &w), "#REF!");
}

#[test]
fn error_propagates_through_references() {
    // A1 errors (div by zero); B1 references A1 and should inherit the error.
    let w = wb(&[(0, 0, "", Some("1/0")), (0, 1, "", Some("A1+1"))]);
    assert_eq!(display(0, 0, &w), "#DIV/0!");
    assert_eq!(display(0, 1, &w), "#DIV/0!");
}

// ── Display formatting ────────────────────────────────────────────────────────────

#[test]
fn plain_cell_displays_raw_value() {
    let w = wb(&[(0, 0, "hello", None)]);
    assert_eq!(display(0, 0, &w), "hello");
}

#[test]
fn number_format_applies_to_display() {
    let currency = CellStyle {
        num_format: NumberFormat::Currency,
        ..Default::default()
    };
    let percent = CellStyle {
        num_format: NumberFormat::Percent,
        align: CellAlign::Right,
        ..Default::default()
    };
    assert_eq!(format_evaluated_value("1234.5", &currency), "$1234.50");
    assert_eq!(format_evaluated_value("0.25", &percent), "25.0%");
    // Non-numeric values pass through unchanged.
    assert_eq!(format_evaluated_value("hello", &currency), "hello");
    // General format is identity.
    let general = CellStyle::default();
    assert_eq!(format_evaluated_value("42", &general), "42");
}
