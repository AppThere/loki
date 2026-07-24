// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end `Application.OpenFileForReading` tests (macro spec §5.3, Phase 7B):
//! a macro reads a picker-chosen file through the gated file path. The test
//! backend stands in for the real app picker: it grants `FileRead`, records the
//! filter it was asked for, and returns a canned file (or `None` for a cancelled
//! pick).

use std::sync::Arc;
use std::sync::Mutex;

use loki_basic::{Dialect, DialogRequest};
use loki_macro_host::{
    Capability, DialogOutcome, FileFilter, FileWriteError, GrantScope, MacroBackend, MacroRuntime,
    PickedFile, RunRequest,
};

/// A backend that answers `FileRead`/`DocWrite` per its flags and returns a
/// canned pick. Records the filter it was handed and whether it was called.
struct FileBackend {
    allow_read: bool,
    canned: Option<PickedFile>,
    seen_filter: Arc<Mutex<Option<Vec<String>>>>,
}

impl MacroBackend for FileBackend {
    fn prompt_capability(&mut self, cap: Capability) -> GrantScope {
        match cap {
            Capability::FileRead if self.allow_read => GrantScope::AllowSession,
            Capability::DocWrite => GrantScope::AllowSession,
            _ => GrantScope::Deny,
        }
    }
    fn show_dialog(&mut self, _req: &DialogRequest) -> DialogOutcome {
        DialogOutcome::Cancelled
    }
    fn read_file(&mut self, filter: &FileFilter) -> Option<PickedFile> {
        *self.seen_filter.lock().unwrap() = Some(filter.extensions.clone());
        self.canned.clone()
    }
}

fn run(src: &str, backend: FileBackend) -> loki_macro_host::RunOutcome {
    MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        RunRequest::new("Doc", "", 10_000_000),
        backend,
    )
}

fn picked(text: &str) -> PickedFile {
    PickedFile {
        path: "/picked/data.txt".to_owned(),
        bytes: text.as_bytes().to_vec(),
    }
}

const READ: &str = "\
Sub Main()
    Dim f As Object
    Set f = Application.OpenFileForReading(\"*.txt\")
    Application.ActiveDocument.AppendText f.Text
End Sub";

#[test]
fn reads_a_picked_file_into_the_document() {
    let seen = Arc::new(Mutex::new(None));
    let backend = FileBackend {
        allow_read: true,
        canned: Some(picked("hello from disk")),
        seen_filter: Arc::clone(&seen),
    };
    let out = run(READ, backend);
    out.result.expect("clean run");
    assert_eq!(out.batch.apply_to(String::new()), "hello from disk");
    // The parsed filter reached the picker.
    assert_eq!(
        seen.lock().unwrap().as_deref(),
        Some(&["txt".to_owned()][..])
    );
}

#[test]
fn denied_fileread_traps_and_never_reaches_the_picker() {
    let seen = Arc::new(Mutex::new(None));
    let backend = FileBackend {
        allow_read: false,
        canned: Some(picked("secret")),
        seen_filter: Arc::clone(&seen),
    };
    let out = run(READ, backend);
    let err = out.result.expect_err("denied FileRead errors");
    assert!(
        !err.is_refusal(),
        "a denied capability is trappable, not a refusal"
    );
    assert!(
        seen.lock().unwrap().is_none(),
        "the picker must not be raised when FileRead is denied"
    );
    assert!(out.batch.is_empty());
}

#[test]
fn a_cancelled_pick_is_a_trappable_error() {
    let seen = Arc::new(Mutex::new(None));
    let backend = FileBackend {
        allow_read: true,
        canned: None, // user cancelled the picker
        seen_filter: Arc::clone(&seen),
    };
    let out = run(READ, backend);
    let err = out.result.expect_err("a cancelled pick errors");
    assert!(
        !err.is_refusal(),
        "a cancelled pick is trappable, not a refusal"
    );
    // The picker *was* raised (the grant succeeded) but returned nothing, so no
    // content reached the document.
    assert!(seen.lock().unwrap().is_some());
    assert!(out.batch.is_empty());
}

#[test]
fn a_cancelled_pick_can_be_trapped_by_the_macro() {
    let seen = Arc::new(Mutex::new(None));
    let backend = FileBackend {
        allow_read: true,
        canned: None,
        seen_filter: Arc::clone(&seen),
    };
    // On Error Resume Next lets the macro recover and record that it failed.
    let src = "Sub Main()\n On Error Resume Next\n \
         Application.OpenFileForReading(\"*.txt\").Text\n \
         Application.ActiveDocument.AppendText CStr(Err.Number <> 0)\nEnd Sub";
    let out = run(src, backend);
    out.result
        .expect("the macro trapped the cancel and finished");
    assert_eq!(out.batch.apply_to(String::new()), "True");
}

// ── FileWrite ────────────────────────────────────────────────────────────────

/// Shared probe of the last filter a picker was handed.
type FilterProbe = Arc<Mutex<Option<Vec<String>>>>;
/// Shared log of `(path, bytes)` flushed through `write_file`.
type WriteLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// A backend that answers `FileWrite`/`DocWrite`, returns a canned save target,
/// and records every flush. `write_result` is what `write_file` returns.
struct WriteBackend {
    allow_write: bool,
    target: Option<String>,
    write_result: Result<(), FileWriteError>,
    seen_filter: FilterProbe,
    written: WriteLog,
}

impl MacroBackend for WriteBackend {
    fn prompt_capability(&mut self, cap: Capability) -> GrantScope {
        match cap {
            Capability::FileWrite if self.allow_write => GrantScope::AllowSession,
            Capability::DocWrite => GrantScope::AllowSession,
            _ => GrantScope::Deny,
        }
    }
    fn show_dialog(&mut self, _req: &DialogRequest) -> DialogOutcome {
        DialogOutcome::Cancelled
    }
    fn pick_write_target(&mut self, filter: &FileFilter) -> Option<String> {
        *self.seen_filter.lock().unwrap() = Some(filter.extensions.clone());
        self.target.clone()
    }
    fn write_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), FileWriteError> {
        self.written
            .lock()
            .unwrap()
            .push((path.to_owned(), bytes.to_vec()));
        self.write_result.clone()
    }
}

fn write_backend(
    allow: bool,
    target: Option<&str>,
    result: Result<(), FileWriteError>,
) -> (WriteBackend, FilterProbe, WriteLog) {
    let seen = Arc::new(Mutex::new(None));
    let written = Arc::new(Mutex::new(Vec::new()));
    let backend = WriteBackend {
        allow_write: allow,
        target: target.map(str::to_owned),
        write_result: result,
        seen_filter: Arc::clone(&seen),
        written: Arc::clone(&written),
    };
    (backend, seen, written)
}

fn run_write(src: &str, backend: WriteBackend) -> loki_macro_host::RunOutcome {
    MacroRuntime::run(
        src,
        Dialect::Vba,
        "Main",
        RunRequest::new("Doc", "", 10_000_000),
        backend,
    )
}

#[test]
fn writes_buffered_text_to_the_picked_target_on_close() {
    let (backend, seen, written) = write_backend(true, Some("/out.txt"), Ok(()));
    let src = "Sub Main()\n Dim f As Object\n \
         Set f = Application.OpenFileForWriting(\"*.txt\")\n \
         f.Write \"hello\"\n f.WriteLine \" world\"\n f.Close\nEnd Sub";
    let out = run_write(src, backend);
    out.result.expect("clean run");
    assert_eq!(
        seen.lock().unwrap().as_deref(),
        Some(&["txt".to_owned()][..])
    );
    let flushed = written.lock().unwrap();
    assert_eq!(flushed.len(), 1);
    assert_eq!(flushed[0].0, "/out.txt");
    assert_eq!(flushed[0].1, b"hello world\n");
}

#[test]
fn denied_filewrite_traps_and_never_raises_the_picker() {
    let (backend, seen, written) = write_backend(false, Some("/out.txt"), Ok(()));
    let src = "Sub Main()\n Application.OpenFileForWriting(\"*.txt\")\nEnd Sub";
    let out = run_write(src, backend);
    let err = out.result.expect_err("denied FileWrite errors");
    assert!(
        !err.is_refusal(),
        "a denied capability is trappable, not a refusal"
    );
    assert!(
        seen.lock().unwrap().is_none(),
        "the save picker must not be raised"
    );
    assert!(written.lock().unwrap().is_empty());
}

#[test]
fn a_cancelled_save_pick_is_trappable_and_writes_nothing() {
    let (backend, seen, written) = write_backend(true, None, Ok(()));
    let src = "Sub Main()\n Application.OpenFileForWriting(\"*.txt\")\nEnd Sub";
    let out = run_write(src, backend);
    out.result.expect_err("a cancelled save pick errors");
    assert!(seen.lock().unwrap().is_some(), "the picker was raised");
    assert!(written.lock().unwrap().is_empty(), "nothing was written");
}

#[test]
fn a_write_failure_on_close_is_trappable() {
    let (backend, _seen, written) = write_backend(
        true,
        Some("/out.txt"),
        Err(FileWriteError::Io("disk full".to_owned())),
    );
    let src = "Sub Main()\n On Error Resume Next\n Dim f As Object\n \
         Set f = Application.OpenFileForWriting(\"*.txt\")\n f.Write \"data\"\n f.Close\n \
         Application.ActiveDocument.AppendText CStr(Err.Number <> 0)\nEnd Sub";
    let out = run_write(src, backend);
    out.result
        .expect("the macro trapped the write failure and finished");
    assert_eq!(out.batch.apply_to(String::new()), "True");
    // The flush was attempted (path + bytes reached the backend) before it failed.
    assert_eq!(written.lock().unwrap().len(), 1);
}

#[test]
fn an_unclosed_write_handle_is_not_flushed() {
    // No `.Close` → the buffered text is never written. Explicit-close contract.
    let (backend, _seen, written) = write_backend(true, Some("/out.txt"), Ok(()));
    let src = "Sub Main()\n Dim f As Object\n \
         Set f = Application.OpenFileForWriting(\"*.txt\")\n f.Write \"lost\"\nEnd Sub";
    let out = run_write(src, backend);
    out.result.expect("clean run");
    assert!(
        written.lock().unwrap().is_empty(),
        "an unclosed handle must not write"
    );
}

#[test]
fn path_and_length_members_read_back() {
    let seen = Arc::new(Mutex::new(None));
    let backend = FileBackend {
        allow_read: true,
        canned: Some(picked("abcde")),
        seen_filter: Arc::clone(&seen),
    };
    let src = "Sub Main()\n Dim f As Object\n \
         Set f = Application.OpenFileForReading()\n \
         Application.ActiveDocument.AppendText f.Path & \":\" & CStr(f.Length)\nEnd Sub";
    let out = run(src, backend);
    out.result.expect("clean run");
    assert_eq!(out.batch.apply_to(String::new()), "/picked/data.txt:5");
    // No filter argument → any file.
    assert_eq!(seen.lock().unwrap().as_deref(), Some(&[][..]));
}
