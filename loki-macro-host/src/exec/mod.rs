// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The execution host (macro spec §4.3, §5, §6) — the [`loki_basic::Host`]
//! implementation that gives a running macro its (gated) authority.
//!
//! [`ExecutionHost`] composes three things:
//!
//! - the [`CapabilityBroker`] — fuel, the shared cancel flag, and the
//!   capability decision for every effect;
//! - a [`MacroBackend`] — the app's seam for the two things the broker cannot
//!   decide alone: prompting the user at first use (spec §5.4) and actually
//!   showing a permitted dialog (spec §5.5);
//! - a document facade — the object-model surface (`Application`,
//!   `ActiveDocument`) over the neutral document, with reads gated by `DocRead`
//!   and writes collected into a single [`EditBatch`] (spec §6.2).
//!
//! Every effect passes through [`ExecutionHost::gate`], so the interpreter can
//! reach nothing the broker has not permitted.

mod edit;
mod facade;
mod file;
mod find;
mod network;

pub use edit::{DocEdit, EditBatch};

use std::collections::BTreeSet;

use find::FindState;
use loki_basic::{DialogRequest, FuelVerdict, Host, ObjectRef, RuntimeError, Value};

use crate::broker::CapabilityBroker;
use crate::capability::{Capability, CapabilityDecision, GrantScope};
use crate::http::{HttpError, HttpRequest};

/// The `Application` object handle.
pub(crate) const APP: ObjectRef = ObjectRef(1);
/// The active-document object handle.
pub(crate) const DOC: ObjectRef = ObjectRef(2);
/// The `Selection`/`Range` object handle (a whole-document text range, §6.1).
pub(crate) const SELECTION: ObjectRef = ObjectRef(3);
/// The `Find` object handle (`Selection.Find`, phase 6).
pub(crate) const FIND: ObjectRef = ObjectRef(4);
/// The `Find.Replacement` object handle (phase 6).
pub(crate) const REPLACEMENT: ObjectRef = ObjectRef(5);
/// Base handle for `HttpResponse` objects (8B.2). A response returned by
/// `Application.HttpGet` is `HTTP_RESPONSE_BASE + index`; well above the fixed
/// singleton handles so there is no collision.
pub(crate) const HTTP_RESPONSE_BASE: u32 = 0x1000;
/// Base handle for picked-file objects (Phase 7B). A file returned by
/// `Application.OpenFileForReading` is `FILE_HANDLE_BASE + index`; a distinct,
/// higher range than [`HTTP_RESPONSE_BASE`] so the two never collide (checked
/// highest-base-first in the facade dispatch).
pub(crate) const FILE_HANDLE_BASE: u32 = 0x2000;

/// The outcome of showing a macro dialog (spec §5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogOutcome {
    /// `MsgBox` — the clicked button's VBA code (e.g. `1` = OK).
    Button(i64),
    /// `InputBox` — the entered text.
    Text(String),
    /// The user dismissed/cancelled the dialog (`InputBox` → `""`).
    Cancelled,
}

/// The app's seam for effects the broker permits but cannot itself perform:
/// prompting the user, and rendering a dialog in the anti-spoof frame.
///
/// A macro run with the [`DenyBackend`] can read the document and compute, but
/// every prompt is denied and every dialog cancelled — the right posture for a
/// spreadsheet UDF (spec §6.3) or a headless preview.
pub trait MacroBackend {
    /// Prompts the user to grant `cap` at first use (spec §5.4) and returns the
    /// scope they chose. The default denies (safe). A real app renders
    /// `AtPermissionPrompt` and, for a persistent/session grant, records it on
    /// the `MacroService`.
    fn prompt_capability(&mut self, _cap: Capability) -> GrantScope {
        GrantScope::Deny
    }

    /// Renders an already-permitted dialog and returns its result.
    fn show_dialog(&mut self, req: &DialogRequest) -> DialogOutcome;

    /// Prompts the user to allow network access to `origin` (ADR-0015 §4.2) and
    /// returns the scope. The default denies. A real app renders the badged
    /// per-host prompt (8B.5) and remembers an `AllowSession` origin for the
    /// session — never to disk.
    fn prompt_network(&mut self, _origin: &str) -> GrantScope {
        GrantScope::Deny
    }

    /// Performs an already-permitted HTTP request (ADR-0015 §4.1). `allowed` is
    /// the set of origins the user has granted this session — the backend follows
    /// redirects **only** within it (the request layer's redirect re-check).
    ///
    /// The default refuses, so any backend that does not opt in (UDF, headless,
    /// tests) has no network. The real app impl ([`crate::net_fetch::NetFetcher`],
    /// 8B.3) enforces HTTPS-only, the header deny-list, no ambient credentials,
    /// and the size/timeout bounds (8B.4).
    fn http_get(
        &mut self,
        _request: &HttpRequest,
        _allowed: &BTreeSet<String>,
    ) -> Result<crate::http::HttpResponse, HttpError> {
        Err(HttpError::Refused)
    }

    /// Raises the OS file picker (filtered by `filter`) and returns the chosen
    /// file's path + bytes, or `None` if the user cancelled (macro spec §5.3,
    /// Phase 7B). The pick *is* the second consent (T3); the [`Capability::FileRead`]
    /// prompt has already been granted by the time this is called.
    ///
    /// The default returns `None` (nothing picked), so a non-interactive backend
    /// (UDF, headless, tests) reads no files.
    fn read_file(&mut self, _filter: &crate::file::FileFilter) -> Option<crate::file::PickedFile> {
        None
    }
}

/// A backend that grants nothing and shows nothing — for UDFs and tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct DenyBackend;

impl MacroBackend for DenyBackend {
    fn show_dialog(&mut self, _req: &DialogRequest) -> DialogOutcome {
        DialogOutcome::Cancelled
    }
}

/// The working state of the document facade during a run.
pub(crate) struct DocFacade {
    /// The document title (`Document.Name`).
    pub(crate) title: String,
    /// The working body text — starts from the document and reflects the run's
    /// own edits so a later `.Text` read sees earlier writes.
    pub(crate) text: String,
    /// The accumulated edits (one undo entry, spec §6.2).
    pub(crate) batch: EditBatch,
    /// Whether the macro requested a print (spec §5.2 `Print`).
    pub(crate) printed: bool,
    /// The `Find`/`Replacement` search state (phase 6). A singleton backing the
    /// stateless `FIND`/`REPLACEMENT` handles.
    pub(crate) find: FindState,
    /// `HttpResponse` objects returned by `Application.HttpGet` this run, indexed
    /// by `handle - HTTP_RESPONSE_BASE` (8B.2).
    pub(crate) responses: Vec<crate::http::HttpResponse>,
    /// Picked-file objects returned by `Application.OpenFileForReading` this run,
    /// indexed by `handle - FILE_HANDLE_BASE` (Phase 7B).
    pub(crate) files: Vec<crate::file::PickedFile>,
}

impl DocFacade {
    /// Stores `response` and returns its object handle.
    pub(crate) fn push_response(&mut self, response: crate::http::HttpResponse) -> ObjectRef {
        let handle = ObjectRef(HTTP_RESPONSE_BASE + self.responses.len() as u32);
        self.responses.push(response);
        handle
    }

    /// Stores `file` and returns its object handle.
    pub(crate) fn push_file(&mut self, file: crate::file::PickedFile) -> ObjectRef {
        let handle = ObjectRef(FILE_HANDLE_BASE + self.files.len() as u32);
        self.files.push(file);
        handle
    }
}

/// The [`loki_basic::Host`] a macro runs against.
pub struct ExecutionHost<B: MacroBackend> {
    broker: CapabilityBroker,
    backend: B,
    doc: DocFacade,
}

impl<B: MacroBackend> ExecutionHost<B> {
    /// Creates a host for a document titled `title` with body `text`, gated by
    /// `broker` and backed by `backend`.
    pub fn new(broker: CapabilityBroker, backend: B, title: String, text: String) -> Self {
        Self {
            broker,
            backend,
            doc: DocFacade {
                title,
                text,
                batch: EditBatch::new(),
                printed: false,
                find: FindState::default(),
                responses: Vec::new(),
                files: Vec::new(),
            },
        }
    }

    /// The edits the run accumulated — the single batch the app applies as one
    /// undo entry (spec §6.2).
    #[must_use]
    pub fn edits(&self) -> &EditBatch {
        &self.doc.batch
    }

    /// Whether the macro requested a print.
    #[must_use]
    pub fn printed(&self) -> bool {
        self.doc.printed
    }

    /// Consumes the host, returning the accumulated edit batch and the backend.
    pub fn into_parts(self) -> (EditBatch, B) {
        (self.doc.batch, self.backend)
    }

    /// Decides whether `cap` may proceed, prompting the user at first use
    /// (spec §5.1, §5.4). Maps the decision to `Ok(())` or a runtime error:
    /// a refused capability is an untrappable feature-refusal; a denied one is a
    /// trappable "permission denied" (70) so a macro can degrade gracefully.
    pub(crate) fn gate(&mut self, cap: Capability) -> Result<(), RuntimeError> {
        match self.broker.evaluate(cap) {
            CapabilityDecision::Granted => Ok(()),
            CapabilityDecision::Refused => Err(RuntimeError::feature_refused(cap.id())),
            CapabilityDecision::Denied => Err(denied(cap)),
            CapabilityDecision::Prompt => {
                let scope = self.backend.prompt_capability(cap);
                if self.broker.apply_prompt(cap, scope) {
                    Ok(())
                } else {
                    Err(denied(cap))
                }
            }
        }
    }
}

impl<B: MacroBackend> Host for ExecutionHost<B> {
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict {
        self.broker.consume_fuel(units)
    }

    fn get_root(&mut self, name: &str) -> Option<ObjectRef> {
        match name.to_ascii_lowercase().as_str() {
            "application" | "thisapplication" => Some(APP),
            "activedocument" | "thisdocument" | "thiscomponent" | "activeworkbook"
            | "thisworkbook" => Some(DOC),
            "selection" | "range" => Some(SELECTION),
            _ => None,
        }
    }

    fn get_member(
        &mut self,
        obj: ObjectRef,
        name: &str,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        facade::get_member(self, obj, name, args)
    }

    fn set_member(&mut self, obj: ObjectRef, name: &str, value: Value) -> Result<(), RuntimeError> {
        facade::set_member(self, obj, name, value)
    }

    fn dialog(&mut self, req: &DialogRequest) -> Result<Value, RuntimeError> {
        self.gate(Capability::UiDialog)?;
        Ok(match self.backend.show_dialog(req) {
            DialogOutcome::Button(code) => Value::from_i64_fit(code),
            DialogOutcome::Text(s) => Value::Str(s),
            DialogOutcome::Cancelled => match req.kind {
                loki_basic::DialogKind::Input => Value::Str(String::new()),
                loki_basic::DialogKind::Message => Value::from_i64_fit(2), // vbCancel
            },
        })
    }
}

/// Accessors used by the facade module (kept here so `DocFacade` stays private).
impl<B: MacroBackend> ExecutionHost<B> {
    pub(crate) fn doc(&self) -> &DocFacade {
        &self.doc
    }

    pub(crate) fn doc_mut(&mut self) -> &mut DocFacade {
        &mut self.doc
    }
}

/// The trappable "permission denied" error a denied capability surfaces as.
fn denied(cap: Capability) -> RuntimeError {
    RuntimeError::new(70, format!("Permission denied: {}", cap.id()))
}
