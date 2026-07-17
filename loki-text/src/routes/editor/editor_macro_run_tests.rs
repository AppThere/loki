// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::{Arc, Mutex};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::get_block_text;
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use loki_doc_model::loro_bridge::document_to_loro;
use loki_macro_host::{Capability, Dialect, MacroService};

use super::{MacroCode, RunMessages, run_macro};
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

/// Builds a document-state + live Loro doc seeded with `paras`.
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

#[test]
fn read_only_macro_runs_without_editing() {
    let (ds, loro) = fixture(&["Hello world"]);
    let svc = MacroService::in_memory();
    let src = "Function Main() As String\n Main = ActiveDocument.Text\nEnd Function";
    let r = run_macro(
        &ds,
        &loro,
        &svc,
        &payload(),
        MacroCode {
            source: src,
            dialect: Dialect::Vba,
            proc: "Main",
        },
        &messages(),
    );
    assert!(r.ok);
    assert!(!r.applied, "a DocRead-only macro makes no edits");
    assert_eq!(r.message, "done");
}

#[test]
fn refused_macro_is_reported_and_makes_no_edits() {
    let (ds, loro) = fixture(&["body"]);
    let svc = MacroService::in_memory();
    let src = "Sub Main()\n Shell \"calc.exe\"\nEnd Sub";
    let r = run_macro(
        &ds,
        &loro,
        &svc,
        &payload(),
        MacroCode {
            source: src,
            dialect: Dialect::Vba,
            proc: "Main",
        },
        &messages(),
    );
    assert!(!r.ok);
    assert_eq!(r.message, "refused");
    assert!(!r.applied);
}

#[test]
fn ungranted_docwrite_is_denied_and_makes_no_edits() {
    let (ds, loro) = fixture(&["keep"]);
    let svc = MacroService::in_memory(); // no DocWrite grant
    let src = "Sub Main()\n ActiveDocument.AppendText \"NO\"\nEnd Sub";
    let r = run_macro(
        &ds,
        &loro,
        &svc,
        &payload(),
        MacroCode {
            source: src,
            dialect: Dialect::Vba,
            proc: "Main",
        },
        &messages(),
    );
    assert!(!r.ok);
    assert_eq!(r.message, "denied");
    assert!(!r.applied);
    assert_eq!(get_block_text(&loro, 0), "keep", "document untouched");
}

#[test]
fn granted_docwrite_applies_edits_end_to_end() {
    let (ds, loro) = fixture(&["Hello"]);
    let svc = MacroService::in_memory();
    let p = payload();
    svc.grant_always(&p, Capability::DocWrite).expect("grant");

    let src = "Sub Main()\n ActiveDocument.AppendText \" world\"\nEnd Sub";
    let r = run_macro(
        &ds,
        &loro,
        &svc,
        &p,
        MacroCode {
            source: src,
            dialect: Dialect::Vba,
            proc: "Main",
        },
        &messages(),
    );
    assert!(r.ok, "granted run should finish: {r:?}");
    assert!(r.applied);
    assert_eq!(r.message, "done-edited");
    // The edit reached the live Loro document.
    assert_eq!(get_block_text(&loro, 0), "Hello world");
}

#[test]
fn dialog_text_is_collected_into_the_log() {
    let (ds, loro) = fixture(&["x"]);
    let svc = MacroService::in_memory();
    let p = payload();
    svc.grant_always(&p, Capability::UiDialog).expect("grant");
    let src = "Sub Main()\n MsgBox \"hello from macro\"\nEnd Sub";
    let r = run_macro(
        &ds,
        &loro,
        &svc,
        &p,
        MacroCode {
            source: src,
            dialect: Dialect::Vba,
            proc: "Main",
        },
        &messages(),
    );
    assert!(r.ok);
    assert_eq!(r.dialog_log, vec!["hello from macro".to_string()]);
}
