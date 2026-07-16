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
    // Keep depth modest so the recursive-descent parser does not overflow the
    // test thread's stack; the point is panic-freedom, not acceptance.
    let src = format!(
        "Sub S\n x = {}1{}\nEnd Sub",
        "(".repeat(200),
        ")".repeat(200)
    );
    let _ = Parser::parse_module(&src, Dialect::Vba);
}
