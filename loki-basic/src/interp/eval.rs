// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Expression evaluation.

use super::Interp;
use super::env::{Frame, key};
use crate::ast::Expr;
use crate::error::RuntimeError;
use crate::host::Host;
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
            Expr::Member { .. } | Expr::New(_) | Expr::WithContext => {
                // The object model (members, `New`, `With` receivers) arrives in
                // a later phase; until then these have no value.
                Err(RuntimeError::new(424, "Object required"))
            }
        }
    }

    /// Resolves a bare name: local variable, constant, module global, or a
    /// zero-argument function call.
    fn eval_var(&mut self, name: &str, frame: &mut Frame) -> Result<Value, RuntimeError> {
        if let Some(v) = frame.get(name) {
            return Ok(v.clone());
        }
        if let Some(v) = self.const_or_global(name) {
            return Ok(v);
        }
        if let Some(&proc) = self.procs.get(&key(name))
            && proc.kind.returns_value()
        {
            return self.invoke_with_values(proc, Vec::new());
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
