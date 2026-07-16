// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The per-call [`Frame`]: local variables, the `On Error` state, the `Err`
//! object, and the function return slot. Frames live on the Rust call stack
//! (one per BASIC procedure invocation), so the interpreter can hold `&mut
//! self` (global state) and `&mut Frame` (the active call) at once without a
//! borrow conflict.

use std::collections::HashMap;

use crate::value::Value;

/// The active `On Error` disposition for a frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum ErrorHandler {
    /// No handler — a runtime error propagates.
    #[default]
    None,
    /// `On Error Resume Next` — continue at the next statement.
    ResumeNext,
    /// `On Error GoTo label` — jump to `label`.
    Label(String),
}

/// The `Err` object state (VBA `Err.Number` / `Err.Description`).
#[derive(Debug, Clone, Default)]
pub(super) struct ErrState {
    /// `Err.Number` — `0` means no error.
    pub number: i32,
    /// `Err.Description`.
    pub description: String,
}

impl ErrState {
    pub(super) fn clear(&mut self) {
        self.number = 0;
        self.description.clear();
    }
}

/// One procedure invocation's mutable state.
pub(super) struct Frame {
    vars: HashMap<String, Value>,
    /// The active error handler.
    pub(super) handler: ErrorHandler,
    /// The `Err` object.
    pub(super) err: ErrState,
    /// Lowercased function/property name, whose variable holds the return value.
    pub(super) ret_key: Option<String>,
    /// Whether string comparisons default to case-insensitive (`Option Compare
    /// Text`).
    pub(super) compare_text: bool,
    /// Stack of `With` receiver objects (for leading-dot access).
    pub(super) with_stack: Vec<Value>,
    /// Body index of the most recent faulting statement, for `Resume`.
    pub(super) resume_pc: Option<usize>,
}

impl Frame {
    /// Creates an empty frame.
    pub(super) fn new(ret_key: Option<String>, compare_text: bool) -> Self {
        Self {
            vars: HashMap::new(),
            handler: ErrorHandler::None,
            err: ErrState::default(),
            ret_key,
            compare_text,
            with_stack: Vec::new(),
            resume_pc: None,
        }
    }

    /// Reads a local variable by name (case-insensitive), if present.
    pub(super) fn get(&self, name: &str) -> Option<&Value> {
        self.vars.get(&key(name))
    }

    /// Mutable access to a local variable, if present.
    pub(super) fn get_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.vars.get_mut(&key(name))
    }

    /// Whether a local variable of this name exists.
    pub(super) fn has(&self, name: &str) -> bool {
        self.vars.contains_key(&key(name))
    }

    /// Sets a local variable, creating it if absent.
    pub(super) fn set(&mut self, name: &str, value: Value) {
        self.vars.insert(key(name), value);
    }

    /// Takes the function's return value (or `Empty` if never assigned).
    pub(super) fn return_value(&self) -> Value {
        self.ret_key
            .as_ref()
            .and_then(|k| self.vars.get(k))
            .cloned()
            .unwrap_or(Value::Empty)
    }
}

/// Normalises an identifier to its case-insensitive lookup key.
pub(super) fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}
