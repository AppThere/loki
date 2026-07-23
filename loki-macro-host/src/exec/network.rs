// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The network verb of the execution host (ADR-0015 §4.1/§4.2, 8B.2), split from
//! `exec/mod.rs` for the 300-line ceiling.
//!
//! `Application.HttpGet` gates the `Network` capability **per origin** (never the
//! generic capability path — a bare "network on" is not a grant), then performs
//! the fetch through the app's [`MacroBackend`]. All state stays private to
//! `exec`; this is a child module, so it reaches the host's fields directly.

use loki_basic::{RuntimeError, Value};

use super::{ExecutionHost, MacroBackend};
use crate::capability::CapabilityDecision;
use crate::http::{HttpError, HttpRequest, origin_of};

impl<B: MacroBackend> ExecutionHost<B> {
    /// `Application.HttpGet(url)` (ADR-0015 §4.1): validate the URL is `https`,
    /// gate the origin, perform the fetch through the backend, and return an
    /// `HttpResponse` object handle. Any failure is a trappable runtime error
    /// except a network-off build/setting, which is the untrappable
    /// feature-refusal.
    pub(crate) fn http_get(&mut self, url: String) -> Result<Value, RuntimeError> {
        let Some(origin) = origin_of(&url) else {
            // Not an absolute https URL — invalid procedure call (5), trappable.
            return Err(RuntimeError::new(
                5,
                "HttpGet requires an absolute https:// URL",
            ));
        };
        self.gate_network(&origin)?;
        let request = HttpRequest {
            url,
            headers: Vec::new(),
        };
        // Snapshot the granted origins *after* gating, so the just-allowed origin
        // (and any prior session grants) bound the backend's redirect following.
        let allowed = self.broker.network_origins();
        match self.backend.http_get(&request, &allowed) {
            Ok(response) => Ok(Value::Object(self.doc.push_response(response))),
            Err(e) => Err(http_error(&e)),
        }
    }

    /// Decides whether a network request to `origin` may proceed, prompting the
    /// user per host at first use (ADR-0015 §4.2).
    fn gate_network(&mut self, origin: &str) -> Result<(), RuntimeError> {
        match self.broker.evaluate_network(origin) {
            CapabilityDecision::Granted => Ok(()),
            CapabilityDecision::Refused => Err(RuntimeError::feature_refused("network")),
            CapabilityDecision::Denied => Err(net_denied(origin)),
            CapabilityDecision::Prompt => {
                let scope = self.backend.prompt_network(origin);
                if self.broker.apply_network_prompt(origin, scope) {
                    Ok(())
                } else {
                    Err(net_denied(origin))
                }
            }
        }
    }
}

/// The trappable "permission denied" error a denied network origin surfaces as.
fn net_denied(origin: &str) -> RuntimeError {
    RuntimeError::new(70, format!("Network access denied: {origin}"))
}

/// Maps an [`HttpError`] to a macro runtime error: a network-off build/setting is
/// the untrappable feature-refusal; every other failure is trappable so a macro
/// can `On Error` around it.
fn http_error(error: &HttpError) -> RuntimeError {
    match error {
        HttpError::Refused => RuntimeError::feature_refused("network"),
        HttpError::Denied => RuntimeError::new(70, "Network access denied".to_string()),
        HttpError::InvalidUrl => RuntimeError::new(5, "Invalid URL".to_string()),
        HttpError::SchemeNotAllowed => {
            RuntimeError::new(5, "Only https:// URLs are allowed".to_string())
        }
        HttpError::TooLarge => RuntimeError::new(7, "Response too large".to_string()),
        HttpError::Timeout => RuntimeError::new(1460, "Network request timed out".to_string()),
        HttpError::Transport(msg) => RuntimeError::new(1004, format!("Network error: {msg}")),
    }
}
