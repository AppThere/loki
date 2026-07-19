// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Phase 6: auto-run event gating (T1 regression corpus) and spreadsheet UDF
//! evaluation (compute-only, `#MACRO!`).

use loki_basic::Value;
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use loki_macro_host::{
    Capability, DenyBackend, Dialect, MacroRuntime, MacroService, RunRequest, UdfOutcome,
    auto_open_handlers,
};

fn payload(tag: &[u8]) -> MacroPayload {
    MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            None,
            tag.to_vec(),
        )],
    )
}

// ── T1: nothing fires without the flag (spec §5.6) ───────────────────────────

#[test]
fn disabled_document_never_authorizes_auto_run() {
    let svc = MacroService::in_memory();
    let p = payload(b"a");
    // Fresh (untrusted) document.
    assert!(svc.authorize_auto_run(&p).is_none());
}

#[test]
fn session_only_trust_never_authorizes_auto_run() {
    let svc = MacroService::in_memory();
    let p = payload(b"b");
    svc.enable_session(&p);
    assert!(svc.is_enabled(&p));
    // Enabled for the session, but auto-run needs a *persistent* opt-in.
    assert!(svc.authorize_auto_run(&p).is_none());
}

#[test]
fn trusted_without_the_flag_never_authorizes_auto_run() {
    let svc = MacroService::in_memory();
    let p = payload(b"c");
    svc.trust_document(&p, None).expect("trust");
    // Trusted, but auto_run_open defaults off.
    assert!(!svc.auto_run_open(&p));
    assert!(svc.authorize_auto_run(&p).is_none());
}

#[test]
fn trusted_with_the_flag_authorizes_auto_run() {
    let svc = MacroService::in_memory();
    let p = payload(b"d");
    svc.trust_document(&p, None).expect("trust");
    svc.set_auto_run_open(&p, true).expect("opt-in");
    assert!(svc.authorize_auto_run(&p).is_some());
}

#[test]
fn revoking_the_flag_revokes_authorization() {
    let svc = MacroService::in_memory();
    let p = payload(b"e");
    svc.trust_document(&p, None).expect("trust");
    svc.set_auto_run_open(&p, true).expect("opt-in");
    assert!(svc.authorize_auto_run(&p).is_some());
    svc.set_auto_run_open(&p, false).expect("opt-out");
    assert!(svc.authorize_auto_run(&p).is_none());
}

#[test]
fn keeping_disabled_after_trust_revokes_authorization() {
    // Trust + opt-in, then "Keep disabled": is_enabled() is false, so no auto-run
    // even though the auto_run_open bit is still set on the record.
    let svc = MacroService::in_memory();
    let p = payload(b"f");
    svc.trust_document(&p, None).expect("trust");
    svc.set_auto_run_open(&p, true).expect("opt-in");
    svc.keep_disabled(&p, None).expect("disable");
    assert!(svc.authorize_auto_run(&p).is_none());
}

#[test]
fn run_event_requires_a_token_and_then_fires() {
    // The only way to call run_event is with a token, which only the flag yields.
    let svc = MacroService::in_memory();
    let p = payload(b"g");
    svc.trust_document(&p, None).expect("trust");
    svc.set_auto_run_open(&p, true).expect("opt-in");
    let token = svc.authorize_auto_run(&p).expect("authorized");

    let mut grants = loki_macro_host::GrantSet::new();
    grants.allow(Capability::DocWrite);
    let src = "Sub Document_Open()\n ActiveDocument.AppendText \"ran\"\nEnd Sub";
    let out = MacroRuntime::run_event(
        src,
        Dialect::Vba,
        "Document_Open",
        RunRequest::new("Doc", "", 1_000_000).with_grants(grants),
        DenyBackend,
        &token,
    );
    out.result.expect("event ran");
    assert_eq!(out.batch.apply_to(String::new()), "ran");
}

#[test]
fn only_open_handlers_are_selected_for_auto_run() {
    // The app asks which procs may fire on open; only the open handlers qualify.
    let procs = ["Main", "Document_Open", "AutoClose", "Helper", "AutoOpen"];
    let open = auto_open_handlers(procs);
    assert_eq!(
        open,
        vec!["Document_Open".to_string(), "AutoOpen".to_string()]
    );
}

// ── UDFs: compute-only, #MACRO! on any effect (spec §6.3) ─────────────────────

#[test]
fn a_pure_udf_returns_its_value() {
    let src = "Function Double(x As Long) As Long\n Double = x * 2\nEnd Function";
    let out = MacroRuntime::eval_udf(src, Dialect::Vba, "Double", vec![Value::Long(21)]);
    assert_eq!(out, UdfOutcome::Value(Value::Long(42)));
}

#[test]
fn a_udf_using_builtins_computes() {
    let src = "Function Up(s As String) As String\n Up = UCase(s)\nEnd Function";
    let out = MacroRuntime::eval_udf(src, Dialect::Vba, "Up", vec![Value::Str("hi".into())]);
    assert_eq!(out, UdfOutcome::Value(Value::Str("HI".into())));
}

#[test]
fn a_udf_reading_the_document_is_macro_error() {
    // Even DocRead is denied in a UDF (spec §6.3: not even DocRead).
    let src = "Function Peek() As String\n Peek = ActiveDocument.Text\nEnd Function";
    let out = MacroRuntime::eval_udf(src, Dialect::Vba, "Peek", vec![]);
    assert_eq!(out, UdfOutcome::Macro);
}

#[test]
fn a_udf_showing_a_dialog_is_macro_error() {
    let src = "Function Ask() As Long\n Ask = MsgBox(\"hi\")\nEnd Function";
    let out = MacroRuntime::eval_udf(src, Dialect::Vba, "Ask", vec![]);
    assert_eq!(out, UdfOutcome::Macro);
}

#[test]
fn a_udf_calling_the_never_list_is_macro_error() {
    let src = "Function Evil() As Long\n Shell \"calc.exe\"\n Evil = 1\nEnd Function";
    let out = MacroRuntime::eval_udf(src, Dialect::Vba, "Evil", vec![]);
    assert_eq!(out, UdfOutcome::Macro);
}

#[test]
fn a_runaway_udf_is_stopped_by_fuel_not_hung() {
    let src = "Function Spin() As Long\n Do\n Loop\nEnd Function";
    let out = MacroRuntime::eval_udf(src, Dialect::Vba, "Spin", vec![]);
    assert_eq!(out, UdfOutcome::Macro);
}

#[test]
fn an_unparseable_udf_is_macro_error() {
    let out = MacroRuntime::eval_udf("Function (((", Dialect::Vba, "X", vec![]);
    assert_eq!(out, UdfOutcome::Macro);
}
