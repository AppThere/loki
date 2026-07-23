// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pure request-shaping policy for the macro `Network` capability (ADR-0015
//! §4.1/§4.3, 8B.3), kept `reqwest`-free so it unit-tests without the `macro-net`
//! feature.
//!
//! Three decisions live here, away from the transport wiring in
//! [`crate::net_fetch`]:
//!
//! - **Header deny-list** ([`sanitized_headers`]) — which author-set request
//!   headers may be sent. Framing / hop-by-hop headers would corrupt the request;
//!   proxy-auth and `Cookie` are ambient-credential channels we never carry
//!   (T4/§4.3). An author-set `Authorization` header *is* allowed — it is the
//!   author's own explicit credential, not an ambient one.
//! - **Redirect re-check** ([`redirect_next`]) — a granted request to origin A
//!   must not be silently redirected to an un-granted origin B. Each hop's origin
//!   is re-checked against the session allow-list; a redirect to a not-yet-granted
//!   origin is refused (a trappable error the macro can re-request), never
//!   silently followed.
//! - **Bounds** — the redirect-hop cap and body-size cap constants.

use std::collections::BTreeSet;

use url::Url;

use crate::http::origin_of;

/// Maximum redirect hops followed before giving up — bounds redirect loops and
/// prompt-spam (ADR-0015 §4.1).
pub(crate) const MAX_REDIRECTS: usize = 5;

/// Response body cap in bytes (16 MiB). A macro fetch is meant for small API
/// payloads, not bulk downloads.
///
// TODO(8B.4): enforce this by streaming with an early cutoff rather than the
// current read-then-measure check, so a hostile server cannot force a large
// allocation before the cap trips; also make the cap configurable per run.
pub(crate) const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Request headers a macro author may never set (matched case-insensitively):
/// framing / hop-by-hop headers that would corrupt the request, and
/// ambient-credential channels (`Cookie`, proxy-auth) we never carry (§4.3).
const DENIED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "connection",
    "transfer-encoding",
    "keep-alive",
    "upgrade",
    "proxy-authorization",
    "proxy-connection",
    "cookie",
];

/// Whether `name` is on the request-header deny-list (case-insensitive).
#[must_use]
pub(crate) fn header_is_denied(name: &str) -> bool {
    DENIED_HEADERS
        .iter()
        .any(|denied| name.eq_ignore_ascii_case(denied))
}

/// The author-set headers that may actually be sent: the input minus the
/// deny-list. The client attaches no cookies or credentials of its own (no cookie
/// store, no default auth), so the only credentials that ever leave are ones the
/// macro author set explicitly (e.g. `Authorization`).
#[must_use]
pub(crate) fn sanitized_headers(headers: &[(String, String)]) -> Vec<(&str, &str)> {
    headers
        .iter()
        .filter(|(name, _)| !header_is_denied(name))
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

/// What the fetch loop should do after receiving a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RedirectNext {
    /// Not a followable redirect — return this response to the caller.
    Stop,
    /// Follow the redirect to this resolved absolute `https` URL (origin allowed).
    Follow(String),
    /// The redirect target's origin is not on the session allow-list — refuse.
    Deny,
    /// The `Location` could not be resolved to an absolute `https` URL — refuse.
    Bad,
}

/// Decide how to proceed after a response with `status` and optional `location`
/// header, relative to the request `base` URL and the session `allowed` origins.
///
/// A non-3xx status, or a 3xx with no `Location`, is [`RedirectNext::Stop`] (the
/// response is returned as-is). A 3xx with a `Location` is resolved against
/// `base`; the result must be an absolute `https` URL whose origin the user has
/// already allowed, else it is [`RedirectNext::Deny`] / [`RedirectNext::Bad`].
#[must_use]
pub(crate) fn redirect_next(
    status: u16,
    location: Option<&str>,
    base: &str,
    allowed: &BTreeSet<String>,
) -> RedirectNext {
    if !(300..400).contains(&status) {
        return RedirectNext::Stop;
    }
    let Some(location) = location else {
        // A 3xx with no Location (e.g. 304 Not Modified) is nothing to follow.
        return RedirectNext::Stop;
    };
    let Some(target) = resolve_redirect(base, location) else {
        return RedirectNext::Bad;
    };
    match origin_of(&target) {
        // Non-https or malformed origin — origin_of already rejects those.
        None => RedirectNext::Bad,
        Some(origin) if allowed.contains(&origin) => RedirectNext::Follow(target),
        Some(_) => RedirectNext::Deny,
    }
}

/// Resolve a possibly-relative `location` against the absolute `base` URL,
/// returning the absolute target as a string, or `None` if either is unparseable.
fn resolve_redirect(base: &str, location: &str) -> Option<String> {
    let base = Url::parse(base).ok()?;
    let target = base.join(location).ok()?;
    Some(target.to_string())
}

#[cfg(test)]
#[path = "net_policy_tests.rs"]
mod tests;
