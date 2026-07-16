// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;

fn kinds(src: &str) -> Vec<TokenKind> {
    Lexer::new(src)
        .tokenize()
        .expect("lex")
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[test]
fn empty_source_is_just_eof() {
    assert_eq!(kinds(""), vec![TokenKind::Eof]);
}

#[test]
fn integers_and_floats() {
    assert_eq!(
        kinds("1 23 12.5 .5 2e3"),
        vec![
            TokenKind::Int(1),
            TokenKind::Int(23),
            TokenKind::Float(12.5),
            TokenKind::Float(0.5),
            TokenKind::Float(2000.0),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn hex_and_octal() {
    assert_eq!(
        kinds("&HFF &H10 &O17"),
        vec![
            TokenKind::Int(255),
            TokenKind::Int(16),
            TokenKind::Int(15),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn type_suffixes_are_discarded() {
    assert_eq!(
        kinds("x% count& price! total# name$"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Ident("count".into()),
            TokenKind::Ident("price".into()),
            TokenKind::Ident("total".into()),
            TokenKind::Ident("name".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn string_with_escaped_quote() {
    assert_eq!(
        kinds(r#""he said ""hi""""#),
        vec![TokenKind::Str(r#"he said "hi""#.into()), TokenKind::Eof]
    );
}

#[test]
fn unterminated_string_is_an_error() {
    assert!(Lexer::new("\"oops").tokenize().is_err());
}

#[test]
fn date_literal_keeps_inner_text() {
    assert_eq!(
        kinds("#1/1/2020#"),
        vec![TokenKind::Date("1/1/2020".into()), TokenKind::Eof]
    );
}

#[test]
fn amp_is_concat_but_amp_h_is_hex() {
    assert_eq!(
        kinds(r#"a & "b""#),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::Amp,
            TokenKind::Str("b".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn comparison_operators() {
    assert_eq!(
        kinds("< <= > >= <> ="),
        vec![
            TokenKind::Lt,
            TokenKind::Le,
            TokenKind::Gt,
            TokenKind::Ge,
            TokenKind::Ne,
            TokenKind::Eq,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn comments_are_skipped() {
    assert_eq!(
        kinds("x = 1 ' this is a comment\n y = 2"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Eq,
            TokenKind::Int(1),
            TokenKind::Newline,
            TokenKind::Ident("y".into()),
            TokenKind::Eq,
            TokenKind::Int(2),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn colon_separates_statements() {
    assert_eq!(
        kinds("x = 1 : y = 2"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Eq,
            TokenKind::Int(1),
            TokenKind::Colon,
            TokenKind::Ident("y".into()),
            TokenKind::Eq,
            TokenKind::Int(2),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn line_continuation_joins_logical_line() {
    assert_eq!(
        kinds("x = 1 + _\n    2"),
        vec![
            TokenKind::Ident("x".into()),
            TokenKind::Eq,
            TokenKind::Int(1),
            TokenKind::Plus,
            TokenKind::Int(2),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn member_access_dot_is_not_a_float() {
    assert_eq!(
        kinds("obj.Value"),
        vec![
            TokenKind::Ident("obj".into()),
            TokenKind::Dot,
            TokenKind::Ident("Value".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn crlf_collapses_to_one_newline() {
    assert_eq!(
        kinds("a\r\nb"),
        vec![
            TokenKind::Ident("a".into()),
            TokenKind::Newline,
            TokenKind::Ident("b".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn spans_cover_the_token_text() {
    let toks = Lexer::new("abc 12").tokenize().unwrap();
    assert_eq!(toks[0].span, Span::new(0, 3));
    assert_eq!(toks[1].span, Span::new(4, 6));
}
