// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cell reference parsing, formula evaluation, and value formatting.

use std::collections::HashSet;

pub(super) const COLS: &[&str] = &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];

/// Helper to parse cell reference (e.g. "B2" -> row=1, col=1)
pub(super) fn parse_cell_ref(s: &str) -> Option<(usize, usize)> {
    let s = s.trim().to_uppercase();
    if s.is_empty() {
        return None;
    }
    let first_char = s.chars().next()?;
    if !first_char.is_ascii_alphabetic() {
        return None;
    }
    let col = (first_char as u32) as i32 - ('A' as u32) as i32;
    if !(0..10).contains(&col) {
        return None;
    }
    let row_str = &s[1..];
    let row = row_str.parse::<usize>().ok()?.checked_sub(1)?;
    if row >= 30 {
        return None;
    }
    Some((row, col as usize))
}

/// Helper to evaluate formulas starting with '=' or cell references in the workbook
pub(super) fn evaluate_cell(
    row: usize,
    col: usize,
    wb: &loki_sheet_model::Workbook,
    visited: &mut HashSet<(usize, usize)>,
) -> String {
    if visited.contains(&(row, col)) {
        return "#REF!".to_string();
    }
    visited.insert((row, col));

    let sheet = match wb.get_sheet(0) {
        Some(s) => s,
        None => {
            visited.remove(&(row, col));
            return "".to_string();
        }
    };

    let cell = match sheet.get_cell(row as u32, col as u32) {
        Some(c) => c,
        None => {
            visited.remove(&(row, col));
            return "".to_string();
        }
    };

    let Some(formula_raw) = &cell.formula else {
        visited.remove(&(row, col));
        return cell.value.clone();
    };

    let formula = formula_raw.trim().to_uppercase();
    let result = if formula.starts_with("SUM(") && formula.ends_with(')') {
        eval_sum(&formula, row, col, wb, visited)
    } else {
        eval_expression(&formula, row, col, wb, visited)
    };

    visited.remove(&(row, col));
    result
}

fn eval_sum(
    formula: &str,
    row: usize,
    col: usize,
    wb: &loki_sheet_model::Workbook,
    visited: &mut HashSet<(usize, usize)>,
) -> String {
    let range_str = &formula[4..formula.len() - 1];
    if let Some((start, end)) = range_str.split_once(':') {
        if let (Some((r1, c1)), Some((r2, c2))) = (parse_cell_ref(start), parse_cell_ref(end)) {
            let mut sum = 0.0;
            let min_r = r1.min(r2);
            let max_r = r1.max(r2);
            let min_c = c1.min(c2);
            let max_c = c1.max(c2);
            for r in min_r..=max_r {
                for c in min_c..=max_c {
                    if (r, c) != (row, col) {
                        let cell_val_str = evaluate_cell(r, c, wb, visited);
                        if let Ok(num) = cell_val_str.parse::<f64>() {
                            sum += num;
                        }
                    }
                }
            }
            sum.to_string()
        } else {
            "#VALUE!".to_string()
        }
    } else {
        "#VALUE!".to_string()
    }
}

fn eval_expression(
    formula: &str,
    _row: usize,
    _col: usize,
    wb: &loki_sheet_model::Workbook,
    visited: &mut HashSet<(usize, usize)>,
) -> String {
    // Simple expression parser for B2-B3-B4 or B2+B3
    let mut tokens_list = Vec::new();
    let mut current_token = String::new();
    for ch in formula.chars() {
        if ch == '+' || ch == '-' {
            if !current_token.trim().is_empty() {
                tokens_list.push(current_token.trim().to_string());
            }
            tokens_list.push(ch.to_string());
            current_token = String::new();
        } else {
            current_token.push(ch);
        }
    }
    if !current_token.trim().is_empty() {
        tokens_list.push(current_token.trim().to_string());
    }

    if tokens_list.is_empty() {
        return "0".to_string();
    }

    let mut total = 0.0;
    let mut next_op = '+';
    let mut first = true;

    for token in tokens_list {
        if token == "+" {
            next_op = '+';
        } else if token == "-" {
            next_op = '-';
        } else {
            let val_f = if let Some((r, c)) = parse_cell_ref(&token) {
                let cell_val_str = evaluate_cell(r, c, wb, visited);
                cell_val_str.parse::<f64>().unwrap_or(0.0)
            } else {
                token.parse::<f64>().unwrap_or(0.0)
            };

            if first {
                total = val_f;
                first = false;
            } else if next_op == '+' {
                total += val_f;
            } else {
                total -= val_f;
            }
        }
    }
    total.to_string()
}

pub(super) fn format_evaluated_value(
    val_str: &str,
    format: &loki_sheet_model::CellStyle,
) -> String {
    if let Ok(num) = val_str.parse::<f64>() {
        match format.num_format {
            loki_sheet_model::NumberFormat::Currency => format!("${:.2}", num),
            loki_sheet_model::NumberFormat::Percent => format!("{:.1}%", num * 100.0),
            loki_sheet_model::NumberFormat::General => val_str.to_string(),
        }
    } else {
        val_str.to_string()
    }
}
