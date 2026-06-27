// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`crate::para_drop_cap`].

use super::*;

#[test]
fn cap_byte_len_single_char() {
    assert_eq!(cap_byte_len("Hello world", DropCapLength::Chars(1)), 1);
}

#[test]
fn cap_byte_len_multi_char() {
    assert_eq!(cap_byte_len("Hello world", DropCapLength::Chars(3)), 3);
}

#[test]
fn cap_byte_len_word_stops_at_space() {
    assert_eq!(cap_byte_len("Hello world", DropCapLength::Word), 5);
}

#[test]
fn cap_byte_len_word_whole_when_no_space() {
    assert_eq!(cap_byte_len("Hello", DropCapLength::Word), 5);
}

#[test]
fn cap_byte_len_skips_leading_whitespace() {
    // Two leading spaces + 'H' → byte length covers the spaces and the char.
    assert_eq!(cap_byte_len("  Hello", DropCapLength::Chars(1)), 3);
}

#[test]
fn cap_byte_len_multibyte_initial() {
    // 'É' is two bytes in UTF-8; one char must yield two bytes.
    assert_eq!(cap_byte_len("École", DropCapLength::Chars(1)), 2);
}

#[test]
fn cap_byte_len_empty_is_zero() {
    assert_eq!(cap_byte_len("", DropCapLength::Chars(1)), 0);
    assert_eq!(cap_byte_len("   ", DropCapLength::Word), 0);
}

#[test]
fn cap_byte_len_chars_zero_treated_as_one() {
    assert_eq!(cap_byte_len("Hello", DropCapLength::Chars(0)), 1);
}
