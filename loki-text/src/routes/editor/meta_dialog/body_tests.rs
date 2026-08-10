// SPDX-License-Identifier: Apache-2.0

//! Tests for the metadata body's presentation helpers.

use super::*;

/// A six-digit word count read digit by digit is a count nobody checks. The
/// separator is a narrow no-break space, so it never wraps mid-number.
#[test]
fn counts_are_grouped_in_threes() {
    assert_eq!(thousands(0), "0");
    assert_eq!(thousands(412), "412");
    assert_eq!(thousands(1_234), "1\u{202F}234");
    assert_eq!(thousands(21_486), "21\u{202F}486");
    assert_eq!(thousands(118_203), "118\u{202F}203");
    assert_eq!(thousands(1_000_000), "1\u{202F}000\u{202F}000");
}

/// The boundary is every third digit from the right, so a group never leads.
#[test]
fn grouping_never_starts_with_a_separator() {
    for n in [1, 12, 123, 1_234, 12_345, 123_456, 1_234_567] {
        assert!(!thousands(n).starts_with('\u{202F}'), "{n}");
        assert_eq!(
            thousands(n).chars().filter(|c| c.is_ascii_digit()).count(),
            n.to_string().len(),
            "{n} loses no digits"
        );
    }
}

/// Only the fields whose storage format is not obvious carry a hint — a hint on
/// every field is a hint on none.
#[test]
fn hints_are_given_where_the_storage_format_is_not_obvious() {
    for field in [
        MetaField::Contributors,
        MetaField::Keywords,
        MetaField::Language,
        MetaField::Issued,
    ] {
        assert!(field_hint(field).is_some(), "{}", field.label());
    }
    for field in [MetaField::Title, MetaField::Creator, MetaField::Rights] {
        assert!(field_hint(field).is_none(), "{}", field.label());
    }
}
