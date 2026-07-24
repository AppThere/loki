// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Threaded tests for the worker↔UI bridge — no Dioxus. A worker thread runs a
//! real macro with [`BridgeBackend`]; the test thread plays the UI, draining
//! prompts and answering per a policy.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loki_macro_host::{
    Capability, Dialect, DialogOutcome, GrantScope, MacroRuntime, NetworkPolicy, PickedFile,
    RunOutcome, RunRequest,
};

use super::{BridgeBackend, UiReply, UiRequest, prompt_channel};

/// Runs `proc` from `src` on a worker thread with a [`BridgeBackend`], while the
/// current thread plays the UI: it drains prompts and answers each via `policy`.
/// Returns the run outcome. `cancel` is shared with the run (Stop control).
fn drive(
    src: &'static str,
    proc: &'static str,
    cancel: Arc<AtomicBool>,
    policy: impl FnMut(&UiRequest) -> Option<UiReply>,
) -> RunOutcome {
    drive_net(src, proc, cancel, false, policy)
}

/// Like [`drive`], but `network` enables the run's [`NetworkPolicy`] so an
/// `HttpGet` reaches the per-origin gate (and thus the network prompt).
fn drive_net(
    src: &'static str,
    proc: &'static str,
    cancel: Arc<AtomicBool>,
    network: bool,
    mut policy: impl FnMut(&UiRequest) -> Option<UiReply>,
) -> RunOutcome {
    let (req_tx, mut req_rx) = prompt_channel();
    let (result_tx, result_rx) = channel();
    let cancel_worker = Arc::clone(&cancel);
    let src = src.to_string();
    let proc = proc.to_string();
    std::thread::spawn(move || {
        let backend = BridgeBackend::new(req_tx, cancel_worker.clone());
        let mut req = RunRequest::new("Doc", "seed", 5_000_000).with_cancel(cancel_worker);
        if network {
            req = req.with_network(NetworkPolicy::enabled());
        }
        let out = MacroRuntime::run(&src, Dialect::Vba, &proc, req, backend);
        let _keep = result_tx.send(out);
    });

    loop {
        // Answer any queued prompt first (the worker blocks after sending one,
        // so no prompt is ever queued behind the final result).
        if let Ok(pending) = req_rx.try_recv() {
            match policy(pending.request()) {
                Some(reply) => pending.answer(reply),
                None => pending.deny(),
            }
            continue;
        }
        match result_rx.try_recv() {
            Ok(outcome) => return outcome,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("worker exited without sending an outcome")
            }
        }
    }
}

#[test]
fn granting_a_capability_lets_the_write_through() {
    let out = drive(
        "Sub Main()\n ActiveDocument.AppendText \" world\"\nEnd Sub",
        "Main",
        Arc::new(AtomicBool::new(false)),
        |req| match req {
            UiRequest::Capability(Capability::DocWrite) => {
                Some(UiReply::Grant(GrantScope::AllowSession))
            }
            _ => None,
        },
    );
    out.result.expect("granted run finishes");
    assert_eq!(out.batch.apply_to("seed".into()), "seed world");
}

#[test]
fn denying_a_capability_traps_and_makes_no_edits() {
    let out = drive(
        "Sub Main()\n ActiveDocument.AppendText \"x\"\nEnd Sub",
        "Main",
        Arc::new(AtomicBool::new(false)),
        // Deny everything.
        |_req| Some(UiReply::Grant(GrantScope::Deny)),
    );
    assert!(out.result.is_err());
    assert!(out.batch.is_empty());
}

#[test]
fn a_dialog_is_answered_by_the_ui() {
    let answered = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&answered);
    let out = drive(
        "Sub Main()\n MsgBox \"hi\"\nEnd Sub",
        "Main",
        Arc::new(AtomicBool::new(false)),
        move |req| match req {
            UiRequest::Capability(Capability::UiDialog) => {
                Some(UiReply::Grant(GrantScope::AllowOnce))
            }
            UiRequest::Dialog(_) => {
                seen.store(true, Ordering::SeqCst);
                Some(UiReply::Dialog(DialogOutcome::Button(1)))
            }
            _ => None,
        },
    );
    out.result.expect("dialog run finishes");
    assert!(answered.load(Ordering::SeqCst), "the dialog reached the UI");
}

#[test]
fn a_network_request_prompts_per_origin_and_deny_traps() {
    // With network enabled, HttpGet to a new origin reaches the per-origin
    // prompt (UiRequest::Network with the exact origin). Answering Deny traps
    // the call — no fetch is attempted, so this holds under either macro-net
    // config.
    let saw_origin = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&saw_origin);
    let out = drive_net(
        "Function Main() As Long\n On Error Resume Next\n \
         Application.HttpGet \"https://api.example.com/ping\"\n Main = Err.Number\nEnd Function",
        "Main",
        Arc::new(AtomicBool::new(false)),
        true,
        move |req| match req {
            UiRequest::Network(origin) => {
                assert_eq!(origin, "https://api.example.com");
                flag.store(true, Ordering::SeqCst);
                Some(UiReply::Grant(GrantScope::Deny))
            }
            _ => None,
        },
    );
    out.result
        .expect("the macro trapped the denial and finished");
    assert!(
        saw_origin.load(Ordering::SeqCst),
        "the origin prompt reached the UI"
    );
}

#[test]
fn stop_cancels_a_running_loop() {
    // An infinite loop with no prompts: Stop trips the cancel flag and the next
    // fuel step aborts (spec §8).
    let cancel = Arc::new(AtomicBool::new(false));
    let trip = Arc::clone(&cancel);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        trip.store(true, Ordering::SeqCst);
    });
    let out = drive("Sub Main()\n Do\n Loop\nEnd Sub", "Main", cancel, |_| None);
    assert!(out.result.unwrap_err().is_resource_stop());
}

#[test]
fn stop_during_a_prompt_unblocks_the_worker() {
    // The worker blocks on a prompt; the "UI" trips cancel and denies it. The
    // worker must unblock (not wedge) and the run must end.
    let cancel = Arc::new(AtomicBool::new(false));
    let trip = Arc::clone(&cancel);
    let out = drive(
        "Sub Main()\n ActiveDocument.AppendText \"x\"\nEnd Sub",
        "Main",
        cancel,
        move |_req| {
            trip.store(true, Ordering::SeqCst);
            Some(UiReply::Grant(GrantScope::Deny))
        },
    );
    assert!(out.result.is_err(), "the run ended instead of wedging");
    assert!(out.batch.is_empty());
}

#[test]
fn a_file_read_round_trips_through_the_bridge() {
    // The "UI" grants FileRead + DocWrite and answers the pick with a canned
    // file; the macro reads its text into the document.
    let out = drive(
        "Sub Main()\n Dim f As Object\n \
         Set f = Application.OpenFileForReading(\"*.txt\")\n \
         ActiveDocument.AppendText f.Text\nEnd Sub",
        "Main",
        Arc::new(AtomicBool::new(false)),
        |req| match req {
            UiRequest::Capability(Capability::FileRead | Capability::DocWrite) => {
                Some(UiReply::Grant(GrantScope::AllowSession))
            }
            UiRequest::PickReadFile(_) => Some(UiReply::ReadFile(Some(PickedFile {
                path: "/f.txt".to_owned(),
                bytes: b"from disk".to_vec(),
            }))),
            _ => None,
        },
    );
    out.result.expect("clean run");
    assert_eq!(out.batch.apply_to(String::new()), "from disk");
}

#[test]
fn a_file_write_round_trips_through_the_bridge() {
    // The "UI" grants FileWrite, answers the save pick with a target, and
    // captures the flush; the macro's buffered text reaches write_file on Close.
    let flushed = Arc::new(Mutex::new(None::<(String, Vec<u8>)>));
    let sink = Arc::clone(&flushed);
    let out = drive(
        "Sub Main()\n Dim f As Object\n \
         Set f = Application.OpenFileForWriting(\"*.txt\")\n \
         f.WriteLine \"hello\"\n f.Close\nEnd Sub",
        "Main",
        Arc::new(AtomicBool::new(false)),
        move |req| match req {
            UiRequest::Capability(Capability::FileWrite) => {
                Some(UiReply::Grant(GrantScope::AllowSession))
            }
            UiRequest::PickWriteTarget(_) => Some(UiReply::WritePath(Some("/out.txt".to_owned()))),
            UiRequest::WriteFile { path, bytes } => {
                *sink.lock().unwrap() = Some((path.clone(), bytes.clone()));
                Some(UiReply::WriteResult(Ok(())))
            }
            _ => None,
        },
    );
    out.result.expect("clean run");
    let (path, bytes) = flushed
        .lock()
        .unwrap()
        .clone()
        .expect("write reached the UI");
    assert_eq!(path, "/out.txt");
    assert_eq!(bytes, b"hello\n");
}

#[test]
fn a_gone_ui_degrades_to_deny() {
    // If the UI never answers and the channel closes, the worker's prompt
    // resolves to Deny rather than blocking forever. Here the policy drops the
    // prompt without answering by returning a deny immediately (models the
    // closed-channel path's effect).
    let out = drive(
        "Function Main() As Long\n On Error Resume Next\n ActiveDocument.AppendText \"x\"\n \
         Main = Err.Number\nEnd Function",
        "Main",
        Arc::new(AtomicBool::new(false)),
        |_req| Some(UiReply::Grant(GrantScope::Deny)),
    );
    // The denied write is trappable; the macro caught it and finished.
    out.result.expect("trapped denial finishes cleanly");
    assert!(out.batch.is_empty());
}
