// SPDX-License-Identifier: Apache-2.0

//! Address validation for the Insert link dialog (design note 22).
//!
//! # Inline and non-blocking
//!
//! A malformed address **dims Insert**; it never throws an alert. The user is
//! mid-typing for most of the time the address is invalid, and an alert on
//! every keystroke that has not yet become a URL is a dialog fighting its own
//! user.

use loki_i18n::fl;

use super::target::LinkKind;

/// The verdict on an address, ready for the line under the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AddressState {
    /// Nothing typed yet — no verdict, and no complaint.
    Empty,
    /// Well-formed for its kind.
    Valid(String),
    /// Not well-formed, with the reason.
    Invalid(String),
}

impl AddressState {
    /// Whether Insert should be live.
    #[must_use]
    pub fn can_insert(&self) -> bool {
        matches!(self, AddressState::Valid(_))
    }

    /// The message under the field, if there is one.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        match self {
            AddressState::Empty => None,
            AddressState::Valid(m) | AddressState::Invalid(m) => Some(m),
        }
    }
}

/// Validates `input` for `kind`, returning the verdict and its message.
#[must_use]
pub(super) fn validate(kind: LinkKind, input: &str) -> AddressState {
    let t = input.trim();
    if t.is_empty() {
        return AddressState::Empty;
    }
    match kind {
        LinkKind::Web => validate_web(t),
        LinkKind::Email => validate_email(t),
        LinkKind::File => AddressState::Valid(fl!("link-dialog-valid-file")),
        // In-document targets are picked, never typed, so an address here is
        // always an anchor this dialog produced.
        LinkKind::Document => AddressState::Valid(fl!("link-dialog-valid-document")),
    }
}

/// The URL the model stores for `input` under `kind`.
///
/// Scheme-less web addresses gain `https://` — a bare `example.org` in a
/// document is a relative path, which is never what someone typing a domain
/// into a link dialog meant.
#[must_use]
pub(super) fn to_url(kind: LinkKind, input: &str) -> String {
    let t = input.trim();
    match kind {
        LinkKind::Web if !has_scheme(t) => format!("https://{t}"),
        LinkKind::Email if strip_mailto(t).len() == t.len() => format!("mailto:{t}"),
        _ => t.to_string(),
    }
}

/// Strips a `mailto:` prefix in any case, the way a reader would read it.
///
/// URI schemes are case-insensitive (RFC 3986 §3.1). Matching only the
/// lowercase spelling let `MAILTO:m@example.org` validate as a bare address —
/// splitting into local `MAILTO:m` and domain `example.org` — and then be
/// prefixed a second time into `mailto:MAILTO:m@example.org`.
#[must_use]
fn strip_mailto(s: &str) -> &str {
    if s.len() >= 7 && s[..7].eq_ignore_ascii_case("mailto:") {
        &s[7..]
    } else {
        s
    }
}

/// Whether `s` already carries a URL scheme.
fn has_scheme(s: &str) -> bool {
    // A scheme is `alpha *( alpha / digit / "+" / "-" / "." ) ":"` (RFC 3986
    // §3.1). Checking for a bare `:` would treat `example.org:8080` as scheme-
    // qualified and leave a port number as the scheme.
    //
    // The dot is excluded here even though the grammar permits it, because
    // `example.org:8080` satisfies the grammar exactly as well as a scheme
    // does, and the two cannot be told apart from the head alone. No scheme a
    // reader will follow — http, https, mailto, file, ftp, tel — contains a
    // dot, and a host that needs a port always does. Excluding it resolves the
    // ambiguity in favour of the case a link dialog actually sees.
    let Some((head, _)) = s.split_once(':') else {
        return false;
    };
    !head.is_empty()
        && head.starts_with(|c: char| c.is_ascii_alphabetic())
        && head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-'))
}

/// Web addresses: an `http`/`https` URL, or a bare host that can become one.
fn validate_web(t: &str) -> AddressState {
    if t.contains(char::is_whitespace) {
        return AddressState::Invalid(fl!("link-dialog-invalid-whitespace"));
    }
    if has_scheme(t) {
        let scheme = t.split(':').next().unwrap_or_default().to_lowercase();
        return match scheme.as_str() {
            "http" | "https" => {
                let rest = t[scheme.len() + 1..].trim_start_matches('/');
                if rest.is_empty() {
                    AddressState::Invalid(fl!("link-dialog-invalid-no-host"))
                } else {
                    AddressState::Valid(fl!("link-dialog-valid-web", scheme = scheme))
                }
            }
            other => AddressState::Invalid(fl!("link-dialog-invalid-scheme", scheme = other)),
        };
    }
    // A bare host: it needs a dot, or it is a word rather than a domain.
    let host = t.split('/').next().unwrap_or_default();
    if host.contains('.') && !host.starts_with('.') && !host.ends_with('.') {
        AddressState::Valid(fl!("link-dialog-valid-web-assumed"))
    } else {
        AddressState::Invalid(fl!("link-dialog-invalid-host"))
    }
}

/// Email addresses: one `@`, something either side, and a dotted domain.
fn validate_email(t: &str) -> AddressState {
    let body = strip_mailto(t);
    if body.contains(char::is_whitespace) {
        return AddressState::Invalid(fl!("link-dialog-invalid-whitespace"));
    }
    let mut parts = body.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return AddressState::Invalid(fl!("link-dialog-invalid-email"));
    };
    if local.is_empty() || domain.is_empty() {
        return AddressState::Invalid(fl!("link-dialog-invalid-email"));
    }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return AddressState::Invalid(fl!("link-dialog-invalid-email-domain"));
    }
    AddressState::Valid(fl!("link-dialog-valid-email"))
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
