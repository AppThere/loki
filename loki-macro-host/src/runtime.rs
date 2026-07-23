// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`MacroRuntime`] — runs one **explicitly-invoked** macro (macro spec §5,
//! §6, Phase 5).
//!
//! The runtime parses a module, builds an [`ExecutionHost`] gated by a
//! [`CapabilityBroker`], and calls a **named** procedure. There is deliberately
//! no auto-run entry point: on-open / `AutoOpen` / `Document_Open` events never
//! fire from here (that is Phase 6, behind the `auto_run_open` opt-in, spec
//! §5.6). The single vector behind essentially all macro malware — code running
//! just because a document opened — is closed by construction.
//!
//! A run returns the document [`EditBatch`] it produced, which the app applies
//! as one Loro transaction = one undo entry (spec §6.2).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use loki_basic::{BasicError, Dialect, Interp, Value, parser::Parser};

use crate::broker::{CapabilityBroker, GrantSet};
use crate::exec::{DenyBackend, EditBatch, ExecutionHost, MacroBackend};

/// Fuel budget for a single UDF call (spec §6.3, §8) — tight, with no continue
/// option: recalc is unattended, so a UDF must be fast or fail.
const UDF_FUEL: u64 = 200_000;

/// Inputs for a single macro run.
pub struct RunRequest {
    /// The host document title (`Document.Name`, and dialog identity).
    pub title: String,
    /// The document body text the macro reads and edits.
    pub text: String,
    /// The capabilities resolved for this run (persisted + session grants).
    pub grants: GrantSet,
    /// The fuel budget (spec §8).
    pub fuel: u64,
    /// The shared cancel flag (the UI Stop control).
    pub cancel: Arc<AtomicBool>,
    /// The origin-scoped network policy (ADR-0015 §4.2). Disabled unless the app
    /// enables it (build feature + runtime setting) and folds in session origins.
    pub network: crate::net::NetworkPolicy,
}

impl RunRequest {
    /// A run request with an empty grant set and the given fuel.
    #[must_use]
    pub fn new(title: impl Into<String>, text: impl Into<String>, fuel: u64) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            grants: GrantSet::new(),
            fuel,
            cancel: Arc::new(AtomicBool::new(false)),
            network: crate::net::NetworkPolicy::disabled(),
        }
    }

    /// Builder: set the resolved grant set.
    #[must_use]
    pub fn with_grants(mut self, grants: GrantSet) -> Self {
        self.grants = grants;
        self
    }

    /// Builder: set the origin-scoped network policy (ADR-0015 §4.2).
    #[must_use]
    pub fn with_network(mut self, network: crate::net::NetworkPolicy) -> Self {
        self.network = network;
        self
    }

    /// Builder: share a cancel flag with the UI.
    #[must_use]
    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = cancel;
        self
    }
}

/// Why a macro run ended other than cleanly.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MacroRunError {
    /// The module could not be parsed (bad source / stomped project).
    #[error("macro could not be parsed: {0}")]
    Parse(String),
    /// A runtime error (or a "never"-list refusal / resource-limit stop).
    #[error("macro error {number}: {message}")]
    Runtime {
        /// VBA error number (`1004` = refused, `1005` = fuel, `1006` = cancelled).
        number: i32,
        /// Human-readable description.
        message: String,
        /// Whether an `On Error` handler could have trapped it.
        trappable: bool,
    },
}

impl MacroRunError {
    /// Whether this error is a "never"-list feature refusal (spec §7).
    #[must_use]
    pub fn is_refusal(&self) -> bool {
        matches!(self, MacroRunError::Runtime { number: 1004, .. })
    }

    /// Whether this error is the fuel-exhausted or cancelled resource stop.
    #[must_use]
    pub fn is_resource_stop(&self) -> bool {
        matches!(
            self,
            MacroRunError::Runtime {
                number: 1005 | 1006,
                ..
            }
        )
    }
}

/// The result of a macro run.
pub struct RunOutcome {
    /// `Ok` on a clean finish; `Err` on parse/runtime failure. Even on error the
    /// edits made **before** the failure are returned (the app decides whether
    /// to apply or discard them).
    pub result: Result<(), MacroRunError>,
    /// The single edit batch the run produced (one undo entry, spec §6.2).
    pub batch: EditBatch,
    /// Whether the macro requested a print.
    pub printed: bool,
}

/// Proof that an auto-run event was authorized (macro spec §5.6).
///
/// This type has **no public constructor** — the only way to obtain one is
/// [`crate::MacroService::authorize_auto_run`], which returns `Some` **only**
/// when the document is trusted *and* the user set the separate `auto_run_open`
/// opt-in. [`MacroRuntime::run_event`] requires a `&AutoRunToken`, so an app
/// cannot fire an on-open/-close/-save handler without first passing that gate.
/// This is the type-level enforcement of "nothing fires without the flag" (T1).
///
/// It is a zero-size marker; `Copy` so the app can pass it to a worker thread.
/// Copying does not weaken the gate — you still had to pass
/// [`crate::MacroService::authorize_auto_run`] to obtain one.
#[derive(Debug, Clone, Copy)]
pub struct AutoRunToken {
    _seal: (),
}

impl AutoRunToken {
    /// Mints a token. Crate-private so only [`crate::MacroService`] (which
    /// checks the flag) can create one.
    pub(crate) fn new() -> Self {
        Self { _seal: () }
    }
}

/// Runs explicitly-invoked macros against a document facade.
pub struct MacroRuntime;

impl MacroRuntime {
    /// Parses `source` and runs the named `proc` against a document facade,
    /// returning the edit batch it produced.
    ///
    /// Only `proc` runs — no event or auto-open discovery (spec §5.6). Capability
    /// decisions and dialogs go through `backend`.
    pub fn run<B: MacroBackend>(
        source: &str,
        dialect: Dialect,
        proc: &str,
        req: RunRequest,
        backend: B,
    ) -> RunOutcome {
        let module = match Parser::parse_module(source, dialect) {
            Ok(m) => m,
            Err(e) => {
                return RunOutcome {
                    result: Err(from_basic(e)),
                    batch: EditBatch::new(),
                    printed: false,
                };
            }
        };
        let broker =
            CapabilityBroker::new(req.grants, req.fuel, req.cancel).with_network(req.network);
        let host = ExecutionHost::new(broker, backend, req.title, req.text);
        let mut interp = match Interp::new(&module, host) {
            Ok(i) => i,
            Err(e) => {
                return RunOutcome {
                    result: Err(from_basic(e)),
                    batch: EditBatch::new(),
                    printed: false,
                };
            }
        };
        let result = interp.call(proc, Vec::new()).map(|_| ());
        let host = interp.into_host();
        let printed = host.printed();
        let (batch, _backend) = host.into_parts();
        RunOutcome {
            result: result.map_err(from_basic),
            batch,
            printed,
        }
    }

    /// Fires an **auto-run event handler** (`Document_Open`, `AutoOpen`, …).
    ///
    /// Identical to [`Self::run`] except it requires an [`AutoRunToken`] — proof
    /// the caller passed [`crate::MacroService::authorize_auto_run`], which only
    /// yields a token for a trusted document with the `auto_run_open` opt-in
    /// (spec §5.6). Without the flag there is no token, so no event fires (T1).
    pub fn run_event<B: MacroBackend>(
        source: &str,
        dialect: Dialect,
        proc: &str,
        req: RunRequest,
        backend: B,
        _authorized: &AutoRunToken,
    ) -> RunOutcome {
        Self::run(source, dialect, proc, req, backend)
    }

    /// Lists the runnable procedure names (`Sub`/`Function`) in `source`, for the
    /// Tools ▸ Macros picker. Returns a parse error if the module is unreadable.
    ///
    /// # Errors
    /// [`MacroRunError::Parse`] if the source cannot be parsed.
    pub fn list_procedures(source: &str, dialect: Dialect) -> Result<Vec<String>, MacroRunError> {
        let module = Parser::parse_module(source, dialect).map_err(from_basic)?;
        Ok(module
            .items
            .iter()
            .filter_map(|item| match item {
                loki_basic::ast::Item::Procedure(p) => Some(p.name.clone()),
                _ => None,
            })
            .collect())
    }
}

/// The result of evaluating a spreadsheet user-defined function (spec §6.3).
#[derive(Debug, Clone, PartialEq)]
pub enum UdfOutcome {
    /// The function computed a value.
    Value(Value),
    /// The function failed — a runtime error, a refused feature, an exhausted
    /// budget, or **any** attempt to reach the outside world (a UDF has zero
    /// capabilities and cannot prompt). The cell shows `#MACRO!`.
    Macro,
}

impl MacroRuntime {
    /// Evaluates a spreadsheet UDF: runs `func` from `source` with `args`,
    /// **compute-only** (spec §6.3). The function gets zero capabilities — not
    /// even `DocRead` — so any object-model access, dialog, or "never"-list call
    /// fails and the cell shows `#MACRO!`. Tight per-call fuel with no continue
    /// option (recalc is unattended); an infinite loop is stopped, not hung.
    #[must_use]
    pub fn eval_udf(source: &str, dialect: Dialect, func: &str, args: Vec<Value>) -> UdfOutcome {
        let Ok(module) = Parser::parse_module(source, dialect) else {
            return UdfOutcome::Macro;
        };
        // A UDF broker denies every effect and cannot prompt (compute-only).
        let broker = CapabilityBroker::for_udf(UDF_FUEL);
        let host = ExecutionHost::new(broker, DenyBackend, String::new(), String::new());
        let Ok(mut interp) = Interp::new(&module, host) else {
            return UdfOutcome::Macro;
        };
        match interp.call(func, args) {
            Ok(value) => UdfOutcome::Value(value),
            Err(_) => UdfOutcome::Macro,
        }
    }
}

/// Maps a `loki-basic` error to a runtime error.
fn from_basic(e: BasicError) -> MacroRunError {
    match e {
        BasicError::Lex { message, .. } | BasicError::Parse { message, .. } => {
            MacroRunError::Parse(message)
        }
        BasicError::Runtime(re) => MacroRunError::Runtime {
            number: re.number,
            message: re.message,
            trappable: re.trappable,
        },
    }
}
