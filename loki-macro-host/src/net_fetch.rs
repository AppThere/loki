// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `reqwest`/`rustls` transport for the macro `Network` capability
//! (ADR-0015 §4.1, 8B.3). Compiled only under the `macro-net` build feature; a
//! distribution built without it has no network code at all (§8 decision 1).
//!
//! [`NetFetcher`] performs a **bounded, read-only** HTTPS GET on behalf of an
//! already-gated origin. All policy — HTTPS-only, the header deny-list, the
//! redirect origin re-check, and the bounds — lives in the pure
//! [`crate::net_policy`] module and is applied here; this module is only the I/O
//! wiring. The client carries **no ambient credentials** (no cookie store, no
//! default auth, no proxy auth) so nothing leaves except what the macro author
//! set explicitly (T4/§4.3).

use std::collections::BTreeSet;
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::header::LOCATION;
use reqwest::redirect::Policy;

use crate::http::{HttpError, HttpRequest, HttpResponse, origin_of};
use crate::net_policy::{
    MAX_BODY_BYTES, MAX_REDIRECTS, RedirectNext, redirect_next, sanitized_headers,
};

/// Default per-request timeout. Applies to the whole request (connect + read).
///
// TODO(8B.4): make this configurable per run and wire the shared cancel flag so
// **Stop** aborts an in-flight fetch, not just the next fuel step.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A blocking HTTPS fetcher for macro network requests. Cheap to hold; a run
/// builds one and reuses it across the (rare, user-gated) fetches a macro makes.
pub struct NetFetcher {
    client: Client,
}

impl NetFetcher {
    /// Build a fetcher. The client follows **no** redirects itself — each hop is
    /// re-gated here against the session allow-list (`redirect(Policy::none())`)
    /// — enforces `https_only`, and attaches no cookies or credentials.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Transport`] if the TLS backend fails to initialise.
    pub fn new() -> Result<Self, HttpError> {
        let client = Client::builder()
            .user_agent(concat!("loki-macro/", env!("CARGO_PKG_VERSION")))
            .redirect(Policy::none())
            .https_only(true)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        Ok(Self { client })
    }

    /// Perform `request`, following only redirects whose origin is in `allowed`.
    ///
    /// The caller (the execution host) has already gated the *initial* origin;
    /// this re-checks it too so the fetcher is safe in isolation, then follows up
    /// to [`MAX_REDIRECTS`] hops, re-checking each. The body is capped at
    /// [`MAX_BODY_BYTES`].
    ///
    /// # Errors
    ///
    /// Trappable [`HttpError`]s: [`HttpError::Denied`] (origin not allowed),
    /// [`HttpError::SchemeNotAllowed`] (non-https), [`HttpError::TooLarge`],
    /// [`HttpError::Timeout`], or [`HttpError::Transport`].
    pub fn fetch(
        &self,
        request: &HttpRequest,
        allowed: &BTreeSet<String>,
    ) -> Result<HttpResponse, HttpError> {
        // The initial URL must be an absolute https URL on an allowed origin.
        let initial_origin = origin_of(&request.url).ok_or(HttpError::SchemeNotAllowed)?;
        if !allowed.contains(&initial_origin) {
            return Err(HttpError::Denied);
        }

        let headers = sanitized_headers(&request.headers);
        let mut url = request.url.clone();

        // Initial request + up to MAX_REDIRECTS follow-ups.
        for _ in 0..=MAX_REDIRECTS {
            let response = self.send(&url, &headers)?;
            let status = response.status().as_u16();
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);

            match redirect_next(status, location.as_deref(), &url, allowed) {
                RedirectNext::Stop => return read_response(response, status),
                RedirectNext::Follow(next) => url = next,
                RedirectNext::Deny => return Err(HttpError::Denied),
                RedirectNext::Bad => {
                    return Err(HttpError::Transport(
                        "unresolvable redirect target".to_string(),
                    ));
                }
            }
        }
        Err(HttpError::Transport("too many redirects".to_string()))
    }

    /// Issue a single GET for `url` with the (already sanitized) `headers`.
    fn send(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, HttpError> {
        let mut builder = self.client.get(url);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.send().map_err(map_reqwest_error)
    }
}

/// Read a terminal response into an [`HttpResponse`], enforcing the size cap.
fn read_response(response: Response, status: u16) -> Result<HttpResponse, HttpError> {
    // Cheap early reject on a declared over-cap length.
    if response
        .content_length()
        .is_some_and(|len| len > MAX_BODY_BYTES as u64)
    {
        return Err(HttpError::TooLarge);
    }
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect();
    let body = response.bytes().map_err(map_reqwest_error)?;
    if body.len() > MAX_BODY_BYTES {
        return Err(HttpError::TooLarge);
    }
    Ok(HttpResponse {
        status,
        headers,
        body: body.to_vec(),
    })
}

/// Map a `reqwest` error to the macro-facing [`HttpError`]. A timeout is its own
/// trappable variant; everything else is a transport error carrying a display
/// message only (never a privileged object).
fn map_reqwest_error(error: reqwest::Error) -> HttpError {
    if error.is_timeout() {
        HttpError::Timeout
    } else {
        HttpError::Transport(error.to_string())
    }
}
