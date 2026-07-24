// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the UI-thread run helpers: `make_run_request` reads the document
//! and grants; `apply_and_report` applies a finished run's batch as one undo
//! entry and builds the report. The interpreter itself is exercised inline (no
//! worker thread needed for these — the bridge has its own threaded tests).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::get_block_text;
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use loki_doc_model::loro_bridge::document_to_loro;
use loki_macro_host::{Capability, Dialect, GrantSet, MacroRuntime, MacroService};

use super::{RunMessages, apply_and_report, make_run_request};
use crate::editing::state::DocumentState;

fn messages() -> RunMessages {
    RunMessages {
        done: "done".into(),
        done_edited: "done-edited".into(),
        refused: "refused".into(),
        denied: "denied".into(),
        stopped: "stopped".into(),
        unreadable: "unreadable".into(),
    }
}

fn payload() -> MacroPayload {
    MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            None,
            b"x".to_vec(),
        )],
    )
}

fn fixture(paras: &[&str]) -> (Arc<Mutex<DocumentState>>, loro::LoroDoc) {
    let mut doc = Document::new();
    doc.sections[0].blocks = paras
        .iter()
        .map(|t| Block::Para(vec![Inline::Str((*t).into())]))
        .collect();
    let loro = document_to_loro(&doc).expect("to loro");
    let mut ds = DocumentState::new();
    ds.document = Some(Arc::new(doc));
    (Arc::new(Mutex::new(ds)), loro)
}

/// A pre-resolved backend that grants whatever is in `allow` (for tests that run
/// the interpreter inline without the bridge's worker thread).
struct GrantBackend {
    allow: Vec<Capability>,
}
impl loki_macro_host::MacroBackend for GrantBackend {
    fn prompt_capability(&mut self, cap: Capability) -> loki_macro_host::GrantScope {
        if self.allow.contains(&cap) {
            loki_macro_host::GrantScope::AllowSession
        } else {
            loki_macro_host::GrantScope::Deny
        }
    }
    fn show_dialog(
        &mut self,
        _req: &loki_macro_host::DialogRequest,
    ) -> loki_macro_host::DialogOutcome {
        loki_macro_host::DialogOutcome::Button(1)
    }
}

#[test]
fn make_run_request_reads_title_body_and_grants() {
    let (ds, _loro) = fixture(&["Hello", "world"]);
    let svc = MacroService::in_memory();
    let p = payload();
    svc.grant_always(&p, Capability::DocWrite).expect("grant");
    let cancel = Arc::new(AtomicBool::new(false));
    let req = make_run_request(&ds, &svc, &p, cancel);
    assert_eq!(req.text, "Hello\nworld");
    assert!(req.grants.contains(Capability::DocWrite));
}

#[test]
fn apply_and_report_applies_a_batch_as_one_undo_entry() {
    let (ds, loro) = fixture(&["Hello"]);
    let svc = MacroService::in_memory();
    let p = payload();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut req = make_run_request(&ds, &svc, &p, cancel);
    // Pre-grant DocWrite so the inline run needs no prompt.
    let mut grants = GrantSet::new();
    grants.allow(Capability::DocWrite);
    req = req.with_grants(grants);

    let src = "Sub Main()\n ActiveDocument.AppendText \" world\"\nEnd Sub";
    let outcome = MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        req,
        GrantBackend {
            allow: vec![Capability::DocWrite],
        },
    );
    let report = apply_and_report(&ds, &loro, outcome, &messages());
    assert!(report.ok && report.applied);
    assert_eq!(report.message, "done-edited");
    assert_eq!(get_block_text(&loro, 0), "Hello world");
}

#[test]
fn apply_and_report_maps_a_refusal() {
    let (ds, loro) = fixture(&["x"]);
    let svc = MacroService::in_memory();
    let p = payload();
    let cancel = Arc::new(AtomicBool::new(false));
    let req = make_run_request(&ds, &svc, &p, cancel);
    let src = "Sub Main()\n Shell \"calc.exe\"\nEnd Sub";
    let outcome = MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        req,
        GrantBackend { allow: vec![] },
    );
    let report = apply_and_report(&ds, &loro, outcome, &messages());
    assert!(!report.ok && !report.applied);
    assert_eq!(report.message, "refused");
    assert_eq!(get_block_text(&loro, 0), "x", "document untouched");
}

#[test]
fn apply_and_report_maps_a_denial() {
    let (ds, loro) = fixture(&["keep"]);
    let svc = MacroService::in_memory();
    let p = payload(); // no DocWrite grant
    let cancel = Arc::new(AtomicBool::new(false));
    let req = make_run_request(&ds, &svc, &p, cancel);
    let src = "Sub Main()\n ActiveDocument.AppendText \"NO\"\nEnd Sub";
    let outcome = MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        req,
        GrantBackend { allow: vec![] },
    );
    let report = apply_and_report(&ds, &loro, outcome, &messages());
    assert_eq!(report.message, "denied");
    assert!(!report.applied);
    assert_eq!(get_block_text(&loro, 0), "keep");
}
