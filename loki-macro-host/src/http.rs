// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The HTTP request/response model for the macro `Network` capability (Phase 8
//! Track B; ADR-0015 §4.1, 8B.2).
//!
//! v1 ships **read-only** `Application.HttpGet(url)` returning an
//! [`HttpResponse`] the macro reads (`.Status`, `.Text`, `.Header("…")`).
//! `HttpPost` is deferred (ADR-0015 §8 decision 3). These are plain data types:
//! the actual request is performed by the app's [`crate::exec::MacroBackend`]
//! (8B.3) using `reqwest`/`rustls`; the interpreter only ever sees the bytes as
//! bytes/string (never parsed into a privileged format, T9).

/// A macro-issued HTTP GET request. Header support is reserved for the request
/// layer (author-set headers minus the deny-list, ADR-0015 §4.3); v1 `HttpGet`
/// sends none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// The absolute `https://` URL.
    pub url: String,
    /// Author-set request headers (empty for v1 `HttpGet`; the backend still
    /// strips the deny-list in 8B.3).
    pub headers: Vec<(String, String)>,
}

/// A completed HTTP response handed back to the macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// The HTTP status code (e.g. `200`).
    pub status: u16,
    /// Response headers, in receipt order.
    pub headers: Vec<(String, String)>,
    /// The response body bytes (size-capped by the backend, 8B.4).
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// The body decoded as UTF-8, lossily. Macros read text APIs; the raw bytes
    /// are never parsed into a privileged format by Loki (T9).
    #[must_use]
    pub fn body_as_string(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The first response header matching `name` (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Why an [`HttpRequest`] could not complete. The shim maps these to macro
/// runtime errors — [`Self::Refused`] to the untrappable feature-refusal, the
/// rest to trappable errors so a macro can degrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// The network capability is off (build feature and/or runtime setting).
    Refused,
    /// The user denied this origin (trappable).
    Denied,
    /// The URL is not a valid absolute `https://` URL.
    InvalidUrl,
    /// A non-`https` scheme was requested (http/ftp/file/… are never allowed).
    SchemeNotAllowed,
    /// The response exceeded the size cap (8B.4).
    TooLarge,
    /// The request timed out (8B.4).
    Timeout,
    /// A transport/TLS error, message for display only.
    Transport(String),
}

/// The normalized **origin** (`https://host[:port]`, host lower-cased) of `url`,
/// or `None` if it is not an absolute `https` URL. Origin is the grant unit
/// (ADR-0015 §4.2). Rejects a URL carrying userinfo (`user@host`) — a common
/// spoofing shape and a channel for ambient-credential smuggling (§4.3).
#[must_use]
pub fn origin_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    // The authority ends at the first path/query/fragment delimiter.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') || authority.contains('\\') {
        return None;
    }
    Some(format!("https://{}", authority.to_ascii_lowercase()))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
