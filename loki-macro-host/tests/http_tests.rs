// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end `Application.HttpGet` tests (ADR-0015 §4.1, 8B.2): a macro fetches
//! a URL through the gated network path and reads the response. The test backend
//! stands in for the real `reqwest` impl (8B.3): it allows a configured origin
//! and returns a canned response.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;

use loki_basic::{Dialect, DialogRequest};
use loki_macro_host::{
    Capability, DialogOutcome, GrantScope, HttpError, HttpRequest, HttpResponse, MacroBackend,
    MacroRuntime, NetworkPolicy, RunRequest,
};

/// A backend that allows one origin, grants `DocWrite` (so the macro can write
/// the response into the doc), and returns a canned 200 body. Records the
/// requested URL so the test can assert it reached the backend.
struct NetBackend {
    allow_origin: Option<String>,
    last_url: Arc<Mutex<Option<String>>>,
}

impl MacroBackend for NetBackend {
    fn prompt_capability(&mut self, cap: Capability) -> GrantScope {
        if cap == Capability::DocWrite {
            GrantScope::AllowSession
        } else {
            GrantScope::Deny
        }
    }
    fn show_dialog(&mut self, _req: &DialogRequest) -> DialogOutcome {
        DialogOutcome::Cancelled
    }
    fn prompt_network(&mut self, origin: &str) -> GrantScope {
        if self.allow_origin.as_deref() == Some(origin) {
            GrantScope::AllowSession
        } else {
            GrantScope::Deny
        }
    }
    fn http_get(
        &mut self,
        request: &HttpRequest,
        allowed: &BTreeSet<String>,
    ) -> Result<HttpResponse, HttpError> {
        // The host passes the granted origins so the backend can bound its
        // redirect following; the just-granted origin must be present.
        assert!(
            allowed.contains("https://api.example.com"),
            "granted origin should be in the allow-list, got {allowed:?}"
        );
        *self.last_url.lock().unwrap() = Some(request.url.clone());
        Ok(HttpResponse {
            status: 200,
            headers: vec![("Content-Type".to_owned(), "text/plain".to_owned())],
            body: b"pong".to_vec(),
        })
    }
}

fn backend(allow: Option<&str>) -> (NetBackend, Arc<Mutex<Option<String>>>) {
    let seen = Arc::new(Mutex::new(None));
    let b = NetBackend {
        allow_origin: allow.map(str::to_owned),
        last_url: Arc::clone(&seen),
    };
    (b, seen)
}

fn run(src: &str, backend: NetBackend, network: NetworkPolicy) -> loki_macro_host::RunOutcome {
    MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        RunRequest::new("Doc", "", 10_000_000).with_network(network),
        backend,
    )
}

const FETCH: &str = "\
Sub Main()
    Dim r As Object
    Set r = Application.HttpGet(\"https://api.example.com/ping\")
    Application.ActiveDocument.AppendText CStr(r.Status)
    Application.ActiveDocument.AppendText r.Text
End Sub";

#[test]
fn http_get_fetches_and_reads_the_response() {
    let (b, seen) = backend(Some("https://api.example.com"));
    let out = run(FETCH, b, NetworkPolicy::enabled());
    out.result.expect("clean run");
    // The exact URL reached the backend.
    assert_eq!(
        seen.lock().unwrap().as_deref(),
        Some("https://api.example.com/ping")
    );
    // The macro read .Status (200) and .Text ("pong") and wrote them to the doc.
    assert_eq!(out.batch.apply_to(String::new()), "200pong");
}

#[test]
fn network_disabled_refuses_untrappably() {
    let (b, seen) = backend(Some("https://api.example.com"));
    let out = run(FETCH, b, NetworkPolicy::disabled());
    let err = out.result.expect_err("network disabled must refuse");
    assert!(
        err.is_refusal(),
        "expected untrappable refusal, got {err:?}"
    );
    assert!(seen.lock().unwrap().is_none(), "backend must not be called");
}

#[test]
fn denied_origin_does_not_reach_the_backend() {
    let (b, seen) = backend(None); // deny every origin on prompt
    let out = run(FETCH, b, NetworkPolicy::enabled());
    let err = out.result.expect_err("denied origin errors");
    assert!(!err.is_refusal(), "denial is trappable, not a refusal");
    assert!(seen.lock().unwrap().is_none());
}

#[test]
fn non_https_url_is_rejected_before_prompting() {
    let (b, seen) = backend(Some("http://api.example.com"));
    let src = "Sub Main()\n  Application.HttpGet \"http://api.example.com/x\"\nEnd Sub";
    let out = run(src, b, NetworkPolicy::enabled());
    assert!(out.result.is_err());
    assert!(seen.lock().unwrap().is_none());
}
