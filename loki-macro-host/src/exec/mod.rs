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
mod find;

pub use edit::{DocEdit, EditBatch};

use find::FindState;
use loki_basic::{DialogRequest, FuelVerdict, Host, ObjectRef, RuntimeError, Value};

use crate::broker::CapabilityBroker;
use crate::capability::{Capability, CapabilityDecision, GrantScope};

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
