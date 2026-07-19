// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Expression evaluation.

use super::Interp;
use super::env::{Frame, key};
use crate::ast::Expr;
use crate::error::RuntimeError;
use crate::host::{Host, ObjectRef};
use crate::value::{Value, binary_op, unary_op};

impl<H: Host> Interp<'_, H> {
    /// Evaluates an expression in `frame`.
    ///
    /// # Errors
    ///
    /// Propagates any runtime error (coercion, division by zero, refusal, …).
    pub(super) fn eval(&mut self, expr: &Expr, frame: &mut Frame) -> Result<Value, RuntimeError> {
        self.step()?;
        match expr {
            Expr::Int(n) => Ok(Value::from_i64_fit(*n)),
            Expr::Float(f) => Ok(Value::Double(*f)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Date(raw) => Ok(parse_date_literal(raw)),
            Expr::Empty => Ok(Value::Empty),
            // No object model in Phase 2: Null and Nothing both read as Null.
            Expr::Null | Expr::Nothing => Ok(Value::Null),
            Expr::Var(name) => self.eval_var(name, frame),
            Expr::Unary { op, operand } => {
                let v = self.eval(operand, frame)?;
                unary_op(*op, &v)
            }
            Expr::Binary { op, lhs, rhs } => {
                let a = self.eval(lhs, frame)?;
                let b = self.eval(rhs, frame)?;
                binary_op(*op, &a, &b, frame.compare_text)
            }
            Expr::Call { callee, args } => self.eval_call(callee, args, frame),
            Expr::Member { object, name } => {
                // The built-in `Err` object (`Err.Number`/`Err.Description`) is
                // interpreter state, not a host object.
                if self.is_err_receiver(object, frame) {
                    return Ok(eval_err_property(name, frame));
                }
                // A refused member name (spec §7) is refused wherever it appears.
                if super::refused::is_refused(name) {
                    return Err(RuntimeError::feature_refused(name));
                }
                // Property read: resolve the receiver, then dispatch. A user
                // class instance (§4.2) is served from the interpreter's own
                // heap, never the host; a host object goes through the seam.
                let recv = self.eval_receiver(object, frame)?;
                if self.is_instance(recv) {
                    return self.instance_get(recv, name);
                }
                self.host.get_member(recv, name, &[])
            }
            // `New` constructs a user class-module instance (§4.2). An unknown
            // class name is an external ProgID/COM object — refused, named (§7).
            Expr::New(class) => self.construct_instance(class),
            Expr::WithContext => frame
                .with_stack
                .last()
                .cloned()
                .ok_or_else(|| RuntimeError::new(91, "Object variable not set")),
        }
    }

    /// Evaluates a member receiver expression to a host object handle, mapping a
    /// non-object to VBA error 424 ("Object required").
    pub(super) fn eval_receiver(
        &mut self,
        object: &Expr,
        frame: &mut Frame,
    ) -> Result<ObjectRef, RuntimeError> {
        let v = self.eval(object, frame)?;
        as_object(&v)
    }

    /// Whether `object` denotes the built-in `Err` object (and is not shadowed by
    /// a user variable of that name).
    pub(super) fn is_err_receiver(&self, object: &Expr, frame: &Frame) -> bool {
        matches!(object, Expr::Var(o)
            if o.eq_ignore_ascii_case("Err")
                && frame.get(o).is_none()
                && self.const_or_global(o).is_none())
    }

    /// Resolves a bare name: local variable, constant, module global, or a
    /// zero-argument function call.
    fn eval_var(&mut self, name: &str, frame: &mut Frame) -> Result<Value, RuntimeError> {
        if let Some(v) = frame.get(name) {
            return Ok(v.clone());
        }
        // Inside a class method, `Me` is the receiver and a bare name resolves
        // against the instance's fields/methods before module-level scope (§4.2).
        if let Some(me) = frame.me {
            if name.eq_ignore_ascii_case("Me") {
                return Ok(Value::Object(me));
            }
            if let Some(v) = self.instance_implicit_get(me, name)? {
                return Ok(v);
            }
        }
        if let Some(v) = self.const_or_global(name) {
            return Ok(v);
        }
        if let Some(&proc) = self.procs.get(&key(name))
            && proc.kind.returns_value()
        {
            return self.invoke_with_values(proc, Vec::new());
        }
        // An object-model root the host exposes (`Application`, `ActiveDocument`,
        // `ThisComponent`, …). Checked after locals/consts/procs so a user
        // variable of the same name always wins.
        if let Some(obj) = self.host.get_root(name) {
            return Ok(Value::Object(obj));
        }
        // A refused "never"-list identifier used as a bare value.
        if super::refused::is_refused(name) {
            return Err(RuntimeError::feature_refused(name));
        }
        // Undeclared: `Option Explicit` rejects; otherwise auto-vivify to Empty.
        if self.explicit() {
            return Err(RuntimeError::new(
                13,
                format!("Variable not defined: {name}"),
            ));
        }
        Ok(Value::Empty)
    }

    pub(super) fn const_or_global(&self, name: &str) -> Option<Value> {
        let k = key(name);
        self.consts
            .get(&k)
            .or_else(|| self.globals.get(&k))
            .cloned()
    }

    fn explicit(&self) -> bool {
        self.options.explicit
    }

    /// Evaluates the index expressions of an array access to `i32` subscripts.
    pub(super) fn eval_indices(
        &mut self,
        args: &[crate::ast::Argument],
        frame: &mut Frame,
    ) -> Result<Vec<i32>, RuntimeError> {
        let mut indices = Vec::with_capacity(args.len());
        for a in args {
            let e = a
                .value
                .as_ref()
                .ok_or_else(RuntimeError::subscript_out_of_range)?;
            indices.push(self.eval(e, frame)?.to_i32()?);
        }
        Ok(indices)
    }
}

/// Reads a property of the built-in `Err` object.
pub(super) fn eval_err_property(name: &str, frame: &Frame) -> Value {
    match name.to_ascii_lowercase().as_str() {
        "number" => Value::from_i64_fit(i64::from(frame.err.number)),
        "description" => Value::Str(frame.err.description.clone()),
        "source" => Value::Str(String::new()),
        _ => Value::Empty,
    }
}

/// Invokes a method of the built-in `Err` object (`Clear` / `Raise`).
pub(super) fn call_err_method(
    name: &str,
    args: &[Value],
    frame: &mut Frame,
) -> Result<Value, RuntimeError> {
    match name.to_ascii_lowercase().as_str() {
        "clear" => {
            frame.err.clear();
            Ok(Value::Empty)
        }
        "raise" => {
            let number = args.first().cloned().unwrap_or(Value::Empty).to_i32()?;
            let desc = match args.get(2) {
                Some(Value::Empty) | None => "Application-defined error".to_string(),
                Some(v) => v.to_basic_string()?,
            };
            Err(RuntimeError::new(number, desc))
        }
        _ => Err(RuntimeError::new(
            438,
            "Object doesn't support this property or method",
        )),
    }
}

/// Maps a value to a host object handle, or VBA error 424 ("Object required")
/// for a non-object (including `Nothing`/`Null`).
pub(super) fn as_object(v: &Value) -> Result<ObjectRef, RuntimeError> {
    match v {
        Value::Object(r) => Ok(*r),
        _ => Err(RuntimeError::new(424, "Object required")),
    }
}

/// Parses a `#…#` date literal to an OLE automation date. Phase 2 supports the
/// common `m/d/yyyy` and bare-number forms; richer parsing lands with the date
/// built-ins.
fn parse_date_literal(raw: &str) -> Value {
    let t = raw.trim();
    if let Ok(n) = t.parse::<f64>() {
        return Value::Date(n);
    }
    if let Some(serial) = super::builtins::parse_us_date(t) {
        return Value::Date(serial);
    }
    Value::Date(0.0)
}
