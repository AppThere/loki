// SPDX-License-Identifier: Apache-2.0

//! Tests for link address validation.

use super::*;

/// An empty box is not an error — the user has not typed yet. It must still
/// leave Insert inert, or an empty address commits.
#[test]
fn an_empty_address_is_neither_valid_nor_complained_about() {
    for kind in LinkKind::ALL {
        let state = validate(kind, "   ");
        assert_eq!(state, AddressState::Empty, "{kind:?}");
        assert!(!state.can_insert());
        assert!(state.message().is_none());
    }
}

#[test]
fn well_formed_web_addresses_are_accepted() {
    for input in [
        "https://appthere.org/loki",
        "http://example.com",
        "HTTPS://EXAMPLE.COM/x",
        "example.org",
        "sub.example.co.uk/path",
    ] {
        assert!(
            validate(LinkKind::Web, input).can_insert(),
            "{input} should be valid"
        );
    }
}

/// The rejections must actually bite — a validator that accepts everything
/// would pass a test that only checked the happy path.
#[test]
fn malformed_web_addresses_are_rejected() {
    for input in [
        "not a url",         // whitespace
        "localhost",         // no dot: a word, not a domain
        "https://",          // scheme with no host
        "ftp://example.org", // scheme we do not link to
        ".example.org",      // leading dot
        "example.org.",      // trailing dot
    ] {
        let state = validate(LinkKind::Web, input);
        assert!(!state.can_insert(), "{input} should be rejected");
        assert!(state.message().is_some(), "{input} should say why");
    }
}

/// A port is not a scheme. `example.org:8080` reads as scheme `example.org`
/// under a naive `contains(':')` check, which would then reject it.
#[test]
fn a_port_number_is_not_mistaken_for_a_scheme() {
    assert!(!has_scheme("example.org:8080"));
    assert!(has_scheme("https://example.org"));
    assert!(has_scheme("mailto:a@b.org"));
    assert!(!has_scheme("no-colon-here"));
    assert!(!has_scheme("1nvalid://x"), "a scheme starts with a letter");
}

/// A bare domain becomes an absolute URL: left alone it is a *relative path*
/// in the exported document, which is never what someone typing a domain meant.
#[test]
fn a_bare_host_gains_an_https_scheme() {
    assert_eq!(to_url(LinkKind::Web, "example.org"), "https://example.org");
    assert_eq!(
        to_url(LinkKind::Web, "  example.org/a  "),
        "https://example.org/a"
    );
}

/// An address that already has a scheme keeps it — including one we would not
/// have added ourselves.
#[test]
fn an_explicit_scheme_is_preserved() {
    assert_eq!(
        to_url(LinkKind::Web, "http://example.org"),
        "http://example.org"
    );
    assert_eq!(
        to_url(LinkKind::File, "file:///home/a.odt"),
        "file:///home/a.odt"
    );
}

#[test]
fn email_addresses_round_trip_through_the_mailto_scheme() {
    assert!(validate(LinkKind::Email, "m.halloran@example.org").can_insert());
    assert!(validate(LinkKind::Email, "mailto:m@example.org").can_insert());
    assert_eq!(
        to_url(LinkKind::Email, "m@example.org"),
        "mailto:m@example.org"
    );
    assert_eq!(
        to_url(LinkKind::Email, "mailto:m@example.org"),
        "mailto:m@example.org",
        "the scheme is not doubled"
    );
}

#[test]
fn malformed_email_addresses_are_rejected() {
    for input in [
        "not-an-email",
        "@example.org",
        "m@",
        "m@localhost",
        "a@b@example.org",
        "m halloran@example.org",
    ] {
        let state = validate(LinkKind::Email, input);
        assert!(!state.can_insert(), "{input} should be rejected");
        assert!(state.message().is_some(), "{input} should say why");
    }
}

/// In-document targets are picked from the outline, so an address here was
/// produced by this dialog and is valid by construction.
#[test]
fn document_anchors_are_valid_by_construction() {
    assert!(validate(LinkKind::Document, "#two-the-long-afternoon").can_insert());
}

/// Every kind maps to exactly one segment, and the indices round-trip — a
/// shared slot would make one kind unreachable.
#[test]
fn kinds_round_trip_through_their_segment_index() {
    for (i, kind) in LinkKind::ALL.iter().enumerate() {
        assert_eq!(kind.index(), i);
        assert_eq!(LinkKind::from_index(i), *kind);
    }
    assert_eq!(LinkKind::from_index(99), LinkKind::File, "saturates");
}

/// Compact abbreviates only the one label that would not fit four-up on a
/// phone; abbreviating the rest would be noise.
#[test]
fn only_the_document_label_abbreviates_at_compact() {
    for kind in LinkKind::ALL {
        let long = kind.label(false);
        let short = kind.label(true);
        if kind == LinkKind::Document {
            assert_ne!(long, short);
            assert!(short.len() <= long.len());
        } else {
            assert_eq!(long, short, "{kind:?} needs no abbreviation");
        }
    }
}
