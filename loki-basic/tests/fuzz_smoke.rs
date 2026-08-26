// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Panic-freedom smoke tests: the lexer and parser must return `Result` (never
//! panic) on malformed, truncated, or adversarial input. This is the in-tree
//! complement to the `cargo-fuzz` targets under `loki-basic/fuzz/` — it runs in
//! ordinary CI without a nightly toolchain (macro spec §12, T9).

use loki_basic::Dialect;
use loki_basic::lexer::Lexer;
use loki_basic::parser::Parser;

/// Inputs crafted to hit lexer/parser edge cases: unterminated literals, deep
/// nesting, stray operators, huge numbers, control chars, and truncations.
const NASTY: &[&str] = &[
    "",
    "\"",
    "#",
    "&H",
    "&O",
    "&HZZZZ",
    "1.2.3",
    "1e",
    "1e+",
    ".",
    "..",
    "Sub",
    "Sub (",
    "Function F(",
    "If Then",
    "For = To",
    "Select Case",
    "Do Loop While",
    "x = = =",
    "((((((((((",
    "))))))))))",
    "a & & b",
    "Dim a(",
    "Dim a(1 To",
    "1234567890123456789012345678901234567890",
    "\"\"\"\"\"\"\"\"",
    "_",
    "   _\n",
    "'comment only",
    ": : : :",
    "Next Next Next",
    "End End End",
    "Property Property",
    "ReDim Preserve",
    "On Error GoTo",
    "MsgBox ,,,,",
    "f(x:=)",
    "\u{0}\u{1}\u{2}",
];

#[test]
fn lexer_never_panics() {
    for src in NASTY {
        // Must not panic; error or token stream both acceptable.
        let _ = Lexer::new(src).tokenize();
    }
}

#[test]
fn parser_never_panics() {
    for src in NASTY {
        let _ = Parser::parse_module(src, Dialect::Vba);
        let _ = Parser::parse_module(src, Dialect::StarBasic);
    }
}

#[test]
fn deeply_nested_expression_does_not_stack_overflow_the_lexer() {
    // The lexer is iterative, so deep nesting is a parser (recursion) concern;
    // still assert the lexer copes with a long input.
    let deep = "(".repeat(5_000);
    let _ = Lexer::new(&deep).tokenize();
}

#[test]
fn moderately_nested_parens_parse_or_error_cleanly() {
    // 200 is over the parser's expression-nesting budget (128 levels), so this
    // is a *rejection* case now; the point is panic-freedom, not acceptance.
    // Genuinely deep input is exercised by the `deeply_nested_*` tests below,
    // which the `parser::depth` guard made safe to write.
    let src = format!(
        "Sub S\n x = {}1{}\nEnd Sub",
        "(".repeat(200),
        ")".repeat(200)
    );
    let _ = Parser::parse_module(&src, Dialect::Vba);
}

#[test]
fn nesting_under_the_budget_still_parses() {
    // Inversion for the depth guard (evidence rule 2): the guard must be *false*
    // for input that is merely unusual. Without this, tightening the budget to
    // zero — rejecting every macro — would pass the whole deep-nesting suite.
    let expr = format!(
        "Sub S\n x = {}1{}\nEnd Sub",
        "(".repeat(100),
        ")".repeat(100)
    );
    Parser::parse_module(&expr, Dialect::Vba).expect("100 parens is inside the budget");

    let mut blocks = String::from("Sub S\n");
    for _ in 0..24 {
        blocks.push_str("If True Then\n");
    }
    blocks.push_str("x = 1\n");
    for _ in 0..24 {
        blocks.push_str("End If\n");
    }
    blocks.push_str("End Sub\n");
    Parser::parse_module(&blocks, Dialect::Vba).expect("24 nested blocks is inside the budget");
}

#[test]
fn deeply_nested_parens_error_instead_of_overflowing_the_stack() {
    // Macros are untrusted input: a hostile module with a few thousand `(` must
    // come back as a `Result::Err`, not abort the host process by exhausting the
    // native stack in the recursive-descent parser.
    let src = format!(
        "Sub S\n x = {}1{}\nEnd Sub",
        "(".repeat(5_000),
        ")".repeat(5_000)
    );
    let err = Parser::parse_module(&src, Dialect::Vba).expect_err("deep nesting must be rejected");
    assert!(
        matches!(&err, loki_basic::BasicError::Parse { message, .. } if message.contains("nested too deeply")),
        "expected a nesting-depth parse error, got {err:?}"
    );
}

#[test]
fn deeply_nested_blocks_error_instead_of_overflowing_the_stack() {
    // Statement nesting recurses too (`parse_block` → `parse_statement` →
    // `parse_if` → `parse_block`), so it needs the same guard.
    let mut src = String::from("Sub S\n");
    for _ in 0..5_000 {
        src.push_str("If True Then\n");
    }
    src.push_str("x = 1\n");
    for _ in 0..5_000 {
        src.push_str("End If\n");
    }
    src.push_str("End Sub\n");
    let err = Parser::parse_module(&src, Dialect::Vba).expect_err("deep nesting must be rejected");
    assert!(
        matches!(&err, loki_basic::BasicError::Parse { message, .. } if message.contains("nested too deeply")),
        "expected a nesting-depth parse error, got {err:?}"
    );
}

#[test]
fn deeply_nested_single_line_ifs_error_instead_of_overflowing_the_stack() {
    // `If a Then If b Then …` recurses through `parse_inline_stmts`, which is a
    // separate cycle from the block reader — it needs the guard independently.
    let src = format!("Sub S\n {}x = 1\nEnd Sub", "If True Then ".repeat(5_000));
    let err = Parser::parse_module(&src, Dialect::Vba).expect_err("deep nesting must be rejected");
    assert!(
        matches!(&err, loki_basic::BasicError::Parse { message, .. } if message.contains("nested too deeply")),
        "expected a nesting-depth parse error, got {err:?}"
    );
}
