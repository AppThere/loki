// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The tree-walking interpreter: evaluates a parsed [`Module`] against a
//! [`Host`]. Fuel-metered (spec §8) and effect-free except through the host.
//!
//! Split by concern: [`env`] (call frames), [`eval`] (expressions), [`exec`] +
//! [`exec_block`] (statements & control flow), [`call`] (procedure invocation
//! with `ByRef` copy-in/out), and [`builtins`] (the pure standard library).

mod builtins;
mod call;
mod env;
mod eval;
mod exec;
mod exec_block;
mod exec_dim;

use std::collections::HashMap;

use crate::ast::{Item, Module, ModuleOptions, Procedure, TypeRef, VarDecl};
use crate::error::{BasicError, RuntimeError};
use crate::host::{FuelVerdict, Host};
use crate::value::{Array, Value};

use env::Frame;

/// Maximum BASIC call-stack depth, a hard guard against unbounded recursion
/// (complements fuel metering; spec §8).
const MAX_CALL_DEPTH: usize = 256;

/// Non-local control-flow outcomes of executing a statement.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Flow {
    /// Fall through to the next statement.
    Normal,
    /// `Exit For` — break the innermost `For`/`For Each`.
    ExitFor,
    /// `Exit Do` — break the innermost `Do`/`While`.
    ExitDo,
    /// `Exit Sub`/`Function`/`Property` — return from the procedure.
    ExitProc,
    /// `GoTo label` — jump to a label in the procedure body.
    Goto(String),
    /// `Resume` / `Resume 0` — retry the faulting statement.
    ResumeRetry,
    /// `Resume Next` — continue after the faulting statement.
    ResumeNext,
    /// `Resume label` — continue at a label (clearing the error).
    ResumeLabel(String),
    /// `End`/`Stop` — halt all execution.
    Halt,
}

/// A tree-walking interpreter over a single module.
pub struct Interp<'m, H: Host> {
    module: &'m Module,
    procs: HashMap<String, &'m Procedure>,
    consts: HashMap<String, Value>,
    globals: HashMap<String, Value>,
    options: ModuleOptions,
    host: H,
    call_depth: usize,
}

impl<'m, H: Host> Interp<'m, H> {
    /// Prepares an interpreter for `module`: indexes procedures, evaluates
    /// module-level constants and `Enum` members, and initialises module
    /// variables.
    ///
    /// # Errors
    ///
    /// Returns [`BasicError`] if a constant/enum/array-bound expression is
    /// invalid.
    pub fn new(module: &'m Module, host: H) -> Result<Self, BasicError> {
        let mut interp = Self {
            module,
            procs: HashMap::new(),
            consts: HashMap::new(),
            globals: HashMap::new(),
            options: module.options,
            host,
            call_depth: 0,
        };
        interp.index_items()?;
        Ok(interp)
    }

    fn index_items(&mut self) -> Result<(), BasicError> {
        for item in &self.module.items {
            match item {
                Item::Procedure(p) => {
                    self.procs.insert(env::key(&p.name), p);
                }
                Item::Const(decls) => {
                    for d in decls {
                        let v = self.eval_const(&d.value)?;
                        self.consts.insert(env::key(&d.name), v);
                    }
                }
                Item::Enum(e) => self.index_enum(e)?,
                Item::Var(decls) => {
                    for d in decls {
                        let v = self.default_value(d)?;
                        self.globals.insert(env::key(&d.name), v);
                    }
                }
                Item::Type(_) | Item::ForeignDecl { .. } => {}
            }
        }
        Ok(())
    }

    fn index_enum(&mut self, e: &crate::ast::EnumDef) -> Result<(), BasicError> {
        let mut next: i64 = 0;
        for (name, value) in &e.members {
            let v = if let Some(expr) = value {
                self.eval_const(expr)?.to_i64_round().map_err(runtime)?
            } else {
                next
            };
            self.consts.insert(env::key(name), Value::from_i64_fit(v));
            next = v + 1;
        }
        Ok(())
    }

    /// Calls a `Sub`/`Function`/`Property Get` by name with the given argument
    /// values, returning its result (`Empty` for a `Sub`).
    ///
    /// # Errors
    ///
    /// Returns [`BasicError::Runtime`] on any runtime error, feature refusal, or
    /// resource-limit stop.
    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, BasicError> {
        let proc = *self.procs.get(&env::key(name)).ok_or_else(|| {
            BasicError::Runtime(RuntimeError::new(35, "Sub or Function not defined"))
        })?;
        match self.invoke_with_values(proc, args) {
            Ok(v) => Ok(v),
            // `End`/`Stop` is a clean halt, not an error.
            Err(e) if e.is_halt() => Ok(Value::Empty),
            Err(e) => Err(BasicError::Runtime(e)),
        }
    }

    /// Access to the host (e.g. to read remaining fuel after a run).
    pub fn host(&self) -> &H {
        &self.host
    }

    // ── Fuel ────────────────────────────────────────────────────────────────

    /// Charges one unit of fuel per interpreter step.
    pub(super) fn step(&mut self) -> Result<(), RuntimeError> {
        match self.host.consume_fuel(1) {
            FuelVerdict::Continue => Ok(()),
            FuelVerdict::Exhausted => Err(RuntimeError::fuel_exhausted()),
            FuelVerdict::Cancelled => Err(RuntimeError::cancelled()),
        }
    }

    pub(super) fn compare_text(&self) -> bool {
        self.options.compare_text
    }

    pub(super) fn option_base(&self) -> i32 {
        self.options.base
    }

    // ── Module-level value construction ─────────────────────────────────────

    /// Evaluates a constant expression in an empty scope.
    fn eval_const(&mut self, expr: &crate::ast::Expr) -> Result<Value, BasicError> {
        let mut scratch = Frame::new(None, self.options.compare_text);
        self.eval(expr, &mut scratch).map_err(runtime)
    }

    /// The initial value for a declared variable (array or typed scalar).
    fn default_value(&mut self, decl: &VarDecl) -> Result<Value, BasicError> {
        if let Some(bounds) = &decl.bounds {
            if bounds.is_empty() {
                // Dynamic array, not yet sized.
                return Array::new(Vec::new()).map(Value::Array).map_err(runtime);
            }
            let mut dims = Vec::with_capacity(bounds.len());
            for b in bounds {
                let lo = match &b.lower {
                    Some(e) => self.eval_const(e)?.to_i32().map_err(runtime)?,
                    None => self.options.base,
                };
                let hi = self.eval_const(&b.upper)?.to_i32().map_err(runtime)?;
                dims.push((lo, hi));
            }
            return Array::new(dims).map(Value::Array).map_err(runtime);
        }
        Ok(typed_default(&decl.ty))
    }
}

/// The zero value for a declared scalar type. `Variant`/unknown → `Empty`.
pub(super) fn typed_default(ty: &TypeRef) -> Value {
    let TypeRef::Named(name) = ty else {
        return Value::Empty;
    };
    match name.to_ascii_lowercase().as_str() {
        "integer" | "byte" => Value::Int(0),
        "long" | "longlong" => Value::Long(0),
        "single" | "double" | "currency" => Value::Double(0.0),
        "boolean" => Value::Bool(false),
        "string" => Value::Str(String::new()),
        "date" => Value::Date(0.0),
        _ => Value::Empty,
    }
}

fn runtime(e: RuntimeError) -> BasicError {
    BasicError::Runtime(e)
}
