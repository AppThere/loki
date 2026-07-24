// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Numeric-literal scanning for the lexer: decimal integers/floats (with
//! `E`/`D` exponents) and `&H` hex / `&O` octal integers. Split from
//! `lexer/mod.rs` for the 300-line ceiling.

use super::{Lexer, TokenKind};
use crate::error::{BasicError, Span};

/// Scans a numeric literal starting at the current cursor position.
///
/// The caller guarantees the current char begins a number (a digit, a `.`
/// followed by a digit, or `&H`/`&O`). Consumes any trailing type-suffix char.
pub(super) fn scan_number(lex: &mut Lexer, start: usize) -> Result<TokenKind, BasicError> {
    if lex.peek() == Some('&') {
        return scan_radix(lex, start);
    }
    scan_decimal(lex, start)
}

/// `&H…` hex / `&O…` octal integer.
fn scan_radix(lex: &mut Lexer, start: usize) -> Result<TokenKind, BasicError> {
    lex.bump(); // '&'
    let (radix, name) = match lex.peek() {
        Some('h' | 'H') => (16u32, "hex"),
        Some('o' | 'O') => (8u32, "octal"),
        _ => {
            return Err(err(start, lex.offset(), "malformed &-radix literal"));
        }
    };
    lex.bump(); // radix letter
    let digits_start = lex.offset();
    let mut text = String::new();
    while let Some(c) = lex.peek() {
        if c.is_ascii_alphanumeric() {
            text.push(c);
            lex.bump();
        } else {
            break;
        }
    }
    if text.is_empty() {
        return Err(err(
            start,
            lex.offset(),
            "missing digits after &-radix prefix",
        ));
    }
    let value = i64::from_str_radix(&text, radix).map_err(|_| {
        err(
            digits_start,
            lex.offset(),
            format!("invalid {name} literal"),
        )
    })?;
    lex.consume_type_suffix();
    Ok(TokenKind::Int(value))
}

/// Decimal integer or float, with optional `E`/`D` exponent.
fn scan_decimal(lex: &mut Lexer, start: usize) -> Result<TokenKind, BasicError> {
    let mut text = String::new();
    let mut is_float = false;

    take_digits(lex, &mut text);

    // Fractional part — only when a digit follows the dot, so `obj.member` and
    // `1.ToString`-style member access leave the `.` for the Dot operator.
    if lex.peek() == Some('.') && lex.peek2().is_some_and(|d| d.is_ascii_digit()) {
        is_float = true;
        text.push('.');
        lex.bump();
        take_digits(lex, &mut text);
    }

    // Exponent: E or D (VBA Double marker), optional sign.
    if matches!(lex.peek(), Some('e' | 'E' | 'd' | 'D')) {
        is_float = true;
        text.push('e');
        lex.bump();
        if matches!(lex.peek(), Some('+' | '-')) {
            text.push(lex.peek().unwrap_or('+'));
            lex.bump();
        }
        let exp_digits_before = text.len();
        take_digits(lex, &mut text);
        if text.len() == exp_digits_before {
            return Err(err(start, lex.offset(), "missing exponent digits"));
        }
    }

    let kind = if is_float {
        let v: f64 = text
            .parse()
            .map_err(|_| err(start, lex.offset(), "invalid floating-point literal"))?;
        TokenKind::Float(v)
    } else if let Ok(v) = text.parse::<i64>() {
        TokenKind::Int(v)
    } else {
        // Integer too large for i64 → fall back to Float (VBA widens to Double).
        let v: f64 = text
            .parse()
            .map_err(|_| err(start, lex.offset(), "invalid numeric literal"))?;
        TokenKind::Float(v)
    };
    lex.consume_type_suffix();
    Ok(kind)
}

fn take_digits(lex: &mut Lexer, text: &mut String) {
    while let Some(c) = lex.peek() {
        if c.is_ascii_digit() {
            text.push(c);
            lex.bump();
        } else {
            break;
        }
    }
}

fn err(start: usize, end: usize, message: impl Into<String>) -> BasicError {
    BasicError::Lex {
        message: message.into(),
        span: Span::new(start, end),
    }
}
