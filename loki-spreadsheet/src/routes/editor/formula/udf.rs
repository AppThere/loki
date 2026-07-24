// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spreadsheet user-defined functions (macro spec §6.3).
//!
//! A [`UdfResolver`] indexes the open document's macro modules by procedure name
//! and evaluates one on demand through [`MacroRuntime::eval_udf`] — **compute
//! only**: the UDF gets zero capabilities (not even `DocRead`), cannot prompt,
//! and is fuel-bounded, so recalculation stays pure, fast, and unpromptable.
//! Any object-model access, dialog, "never"-list call, runtime error, or runaway
//! loop yields [`UdfOutcome::Macro`] → the cell shows `#MACRO!`.
//!
//! Because a UDF is safe *by construction*, it needs no trust grant: unlike an
//! executable macro (which is disabled by default), a UDF cannot read the
//! document, reach the network, or touch the filesystem, so evaluating one from
//! any workbook cannot cause harm.

use std::collections::HashMap;
use std::sync::Arc;

use loki_basic::{Dialect, Value};
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use loki_macro_host::{MacroRuntime, UdfOutcome};

/// Indexes a workbook's macro procedures so a cell formula can call one by name.
pub(crate) struct UdfResolver {
    dialect: Dialect,
    /// Lowercased procedure name → the module source that defines it.
    funcs: HashMap<String, Arc<str>>,
}

impl UdfResolver {
    /// Builds a resolver from a preserved macro payload, or `None` if the payload
    /// carries no readable procedures.
    pub(crate) fn from_payload(payload: &MacroPayload) -> Option<Self> {
        let (dialect, modules) = decode(payload)?;
        Self::from_modules(dialect, modules)
    }

    /// Builds a resolver from decoded `(name, source)` modules, indexing every
    /// procedure by name. `None` if no module yields a readable procedure.
    pub(crate) fn from_modules(dialect: Dialect, modules: Vec<(String, String)>) -> Option<Self> {
        let mut funcs: HashMap<String, Arc<str>> = HashMap::new();
        for (_name, source) in &modules {
            let Ok(procs) = MacroRuntime::list_procedures(source, dialect) else {
                continue;
            };
            if procs.is_empty() {
                continue;
            }
            let arc: Arc<str> = Arc::from(source.as_str());
            for p in procs {
                funcs
                    .entry(p.to_ascii_lowercase())
                    .or_insert_with(|| arc.clone());
            }
        }
        (!funcs.is_empty()).then_some(Self { dialect, funcs })
    }

    /// Whether a procedure of this name exists (case-insensitive).
    pub(crate) fn defines(&self, name: &str) -> bool {
        self.funcs.contains_key(&name.to_ascii_lowercase())
    }

    /// Evaluates the UDF `name` with `args`, compute-only. `None` if no such
    /// procedure exists (the caller then reports `#NAME?`).
    pub(crate) fn call(&self, name: &str, args: Vec<Value>) -> Option<UdfOutcome> {
        let source = self.funcs.get(&name.to_ascii_lowercase())?;
        Some(MacroRuntime::eval_udf(source, self.dialect, name, args))
    }
}

/// Decodes the payload's modules to `(name, source)` pairs and their dialect.
fn decode(payload: &MacroPayload) -> Option<(Dialect, Vec<(String, String)>)> {
    match payload.kind {
        MacroPayloadKind::OoxmlVba => {
            let bytes = payload
                .parts
                .iter()
                .find(|p| p.name.ends_with("vbaProject.bin"))
                .map(|p| p.bytes.as_slice())?;
            let project = loki_vba::VbaProject::read(bytes).ok()?;
            let modules = project
                .modules
                .into_iter()
                .map(|m| (m.name, m.source))
                .collect();
            Some((Dialect::Vba, modules))
        }
        MacroPayloadKind::OdfBasic => {
            let modules = loki_odf::basic::extract_basic_modules(payload)
                .into_iter()
                .map(|m| (m.name, m.source))
                .collect();
            Some((Dialect::StarBasic, modules))
        }
    }
}
