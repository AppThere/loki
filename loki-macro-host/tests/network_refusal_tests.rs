// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The **always-refused** network posture for non-interactive contexts
//! (ADR-0015 §8 decision 4/5, 8B.6).
//!
//! Two independent guarantees keep macro network access off everywhere except an
//! interactive client that has opted in (build feature + runtime setting + a
//! per-host grant):
//!
//! 1. **Compile-time.** No server or headless crate links `loki-macro-host` at
//!    all (it depends on the `loki-basic` interpreter, which the dependency
//!    gate — `scripts/check-loki-basic-pure.py` — forbids any server/headless
//!    crate from linking). So the `reqwest`/`rustls` transport is not even
//!    present in those binaries. This test crate cannot assert a *negative*
//!    dependency edge, but the gate does, in CI.
//! 2. **Run-time.** The backend used for every non-interactive run is
//!    [`DenyBackend`], whose `http_get` refuses, and a spreadsheet-UDF run is
//!    additionally compute-only (network disabled, cannot prompt). These are the
//!    tests below.

use std::collections::BTreeSet;

use loki_basic::Dialect;
use loki_macro_host::{
    DenyBackend, HttpError, HttpRequest, MacroBackend, MacroRuntime, UdfOutcome,
};

#[test]
fn deny_backend_refuses_http_get_untrappably() {
    let mut backend = DenyBackend;
    let request = HttpRequest {
        url: "https://api.example.com/x".to_owned(),
        headers: Vec::new(),
    };
    let allowed = BTreeSet::from(["https://api.example.com".to_owned()]);
    // Even with the origin allowed, the non-interactive backend performs no I/O.
    assert_eq!(
        backend.http_get(&request, &allowed),
        Err(HttpError::Refused)
    );
}

#[test]
fn a_udf_calling_httpget_yields_macro_error() {
    // A spreadsheet UDF is compute-only (spec §6.3): zero capabilities, network
    // disabled, and it cannot prompt — so `HttpGet` fails and the cell shows
    // `#MACRO!` rather than reaching the network. This is the recalc / headless
    // posture.
    let source = "Function Ping() As String\n \
         Ping = Application.HttpGet(\"https://api.example.com/x\").Text\nEnd Function";
    let outcome = MacroRuntime::eval_udf(source, Dialect::Vba, "Ping", Vec::new());
    assert!(
        matches!(outcome, UdfOutcome::Macro),
        "a UDF must never reach the network, got {outcome:?}"
    );
}
