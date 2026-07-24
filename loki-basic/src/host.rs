// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`Host`] seam — the interpreter's *only* channel to the outside world.
//!
//! The interpreter has no ambient authority (macro spec §4.3): it can evaluate
//! expressions and mutate its own heap, but anything observable — resource
//! accounting today; dialogs, document reads/writes, and the object model in
//! later phases — is routed through a `Host`. A `loki-basic` embedded with the
//! [`NullHost`] is a pure calculator with unlimited fuel; a real embedding
//! supplies a metering, capability-brokering host (`loki-macro-host`).
//!
//! Phase 5 adds the effect surface on top of the fuel channel: object-model
//! roots and member get/set/call ([`Host::get_root`], [`Host::get_member`],
//! [`Host::set_member`]) and dialogs ([`Host::dialog`]). Every method has a
//! default **deny** implementation, so the pure-calculator hosts ([`NullHost`],
//! [`FuelBudget`]) stay unchanged and a real embedding overrides only what it
//! offers. The interpreter still has no ambient authority — it can only reach
//! these channels, and a host that overrides nothing exposes nothing.

use crate::error::RuntimeError;
use crate::value::Value;

/// The host's verdict when the interpreter asks to spend fuel (spec §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelVerdict {
    /// Fuel remained; keep executing.
    Continue,
    /// The budget is exhausted — the interpreter must stop with an untrappable
    /// [`crate::RuntimeError::fuel_exhausted`].
    Exhausted,
    /// The host cancelled execution (user pressed Stop). The interpreter stops
    /// with an untrappable [`crate::RuntimeError::cancelled`].
    Cancelled,
}

/// An opaque handle to a host object (spec §4.3). The host assigns the
/// identity; the interpreter only ever passes it back through [`Host`] methods
/// and compares handles for the `Is` operator. The interpreter never
/// dereferences it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectRef(pub u32);

/// Which kind of dialog a macro is requesting (spec §5.2 `UiDialog`, §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    /// `MsgBox` — show a message; the reply is the clicked button code.
    Message,
    /// `InputBox` — prompt for text; the reply is the entered string (or `""`
    /// on cancel).
    Input,
}

/// A macro-originated dialog request (`MsgBox`/`InputBox`), routed to the host
/// so it can gate it against the `UiDialog` capability and render it in the
/// anti-spoof frame (spec §5.5). The interpreter constructs this; the host
/// decides whether and how to show it.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogRequest {
    /// Message or input dialog.
    pub kind: DialogKind,
    /// The prompt text.
    pub prompt: String,
    /// The `MsgBox` buttons/flags argument, or `0` for `InputBox`.
    pub buttons: i64,
    /// The optional title (second/third argument).
    pub title: Option<String>,
    /// The optional `InputBox` default text.
    pub default: Option<String>,
}

/// The interpreter's sole interface to its embedding.
///
/// Implementors decide how much work a macro may do (fuel) and which effects it
/// may perform. The interpreter calls [`Host::consume_fuel`] as it walks the
/// tree; returning anything other than [`FuelVerdict::Continue`] halts the run.
/// The object-model and dialog methods default to a **deny**, so a host grants
/// authority only by overriding them.
pub trait Host {
    /// Accounts `units` of work against the budget and reports whether
    /// execution may continue. Called frequently (per statement / loop
    /// iteration / expression step), so implementations must be cheap.
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict;

    /// Resolves a global object-model root name (`Application`,
    /// `ActiveDocument`, `ThisComponent`, …) to a host object, or `None` if the
    /// name is not a root the host exposes. Called only after local variables,
    /// constants, procedures, and built-ins have been ruled out.
    fn get_root(&mut self, _name: &str) -> Option<ObjectRef> {
        None
    }

    /// Reads a property (`args` empty) or calls a method (`args` non-empty) on a
    /// host object. The host gates the effect against its capability grants and
    /// returns the result, or a runtime error (a trappable denial, or an
    /// untrappable feature refusal).
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when the member does not exist, the arguments
    /// are wrong, or a required capability is denied/refused.
    fn get_member(
        &mut self,
        _obj: ObjectRef,
        _name: &str,
        _args: &[Value],
    ) -> Result<Value, RuntimeError> {
        Err(no_member())
    }

    /// Assigns a property on a host object (`obj.Name = value`).
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when the property does not exist or is
    /// read-only, or a required capability is denied/refused.
    fn set_member(
        &mut self,
        _obj: ObjectRef,
        _name: &str,
        _value: Value,
    ) -> Result<(), RuntimeError> {
        Err(no_member())
    }

    /// Shows a macro-originated dialog and returns its reply (`MsgBox` → a
    /// button code as an `Integer`; `InputBox` → the entered `String`).
    ///
    /// # Errors
    ///
    /// Returns a [`RuntimeError`] when the `UiDialog` capability is denied
    /// (trappable) or the host cannot show the dialog.
    fn dialog(&mut self, _req: &DialogRequest) -> Result<Value, RuntimeError> {
        Err(RuntimeError::new(70, "Permission denied"))
    }
}

/// The standard "object doesn't support this property or method" error (438).
fn no_member() -> RuntimeError {
    RuntimeError::new(438, "Object doesn't support this property or method")
}

/// A host that grants unlimited fuel and no capabilities — the "pure
/// calculator" embedding used for expression evaluation and tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullHost;

impl Host for NullHost {
    fn consume_fuel(&mut self, _units: u64) -> FuelVerdict {
        FuelVerdict::Continue
    }
}

/// A simple fuel-metering host: grants a fixed budget, then reports
/// [`FuelVerdict::Exhausted`]. Useful for tests and simple embeddings; the real
/// app host also folds in cancellation and a wall-clock watchdog.
#[derive(Debug, Clone, Copy)]
pub struct FuelBudget {
    remaining: u64,
}

impl FuelBudget {
    /// Creates a budget of `total` fuel units.
    #[must_use]
    pub fn new(total: u64) -> Self {
        Self { remaining: total }
    }

    /// Fuel units still available.
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}

impl Host for FuelBudget {
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict {
        if let Some(rest) = self.remaining.checked_sub(units) {
            self.remaining = rest;
            FuelVerdict::Continue
        } else {
            self.remaining = 0;
            FuelVerdict::Exhausted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_host_never_exhausts() {
        let mut h = NullHost;
        assert_eq!(h.consume_fuel(u64::MAX), FuelVerdict::Continue);
    }

    #[test]
    fn budget_exhausts_when_spent() {
        let mut h = FuelBudget::new(10);
        assert_eq!(h.consume_fuel(4), FuelVerdict::Continue);
        assert_eq!(h.consume_fuel(6), FuelVerdict::Continue);
        assert_eq!(h.remaining(), 0);
        assert_eq!(h.consume_fuel(1), FuelVerdict::Exhausted);
    }
}
