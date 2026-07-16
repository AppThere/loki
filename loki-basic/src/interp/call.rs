// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Procedure invocation: callee resolution (array index vs. user procedure vs.
//! built-in), argument binding (positional, named, `Optional`, `ParamArray`),
//! and `ByRef` copy-in/copy-out for lvalue arguments.

use super::env::{Frame, key};
use super::{Flow, Interp, MAX_CALL_DEPTH};
use crate::ast::{Argument, Expr, Procedure};
use crate::error::RuntimeError;
use crate::host::Host;
use crate::value::{Array, Value};

impl<H: Host> Interp<'_, H> {
    /// Evaluates a `callee(args)` expression: array index, user call, or
    /// built-in.
    pub(super) fn eval_call(
        &mut self,
        callee: &Expr,
        args: &[Argument],
        frame: &mut Frame,
    ) -> Result<Value, RuntimeError> {
        let Expr::Var(name) = callee else {
            return Err(RuntimeError::new(424, "Object required"));
        };
        // Array element read (local first, then module global).
        if matches!(frame.get(name), Some(Value::Array(_))) {
            let indices = self.eval_indices(args, frame)?;
            if let Some(Value::Array(arr)) = frame.get(name) {
                return arr.get(&indices);
            }
        }
        if let Some(Value::Array(arr)) = self.globals.get(&key(name)).cloned() {
            let indices = self.eval_indices(args, frame)?;
            return arr.get(&indices);
        }
        if let Some(&proc) = self.procs.get(&key(name)) {
            return self.invoke_call(proc, args, frame);
        }
        if super::builtins::is_builtin(name) {
            return self.call_builtin(name, args, frame);
        }
        Err(RuntimeError::new(35, "Sub or Function not defined"))
    }

    /// Invokes `proc` with pre-evaluated argument values (public entry / zero-arg
    /// calls). No `ByRef` copy-out (there are no caller lvalues).
    pub(super) fn invoke_with_values(
        &mut self,
        proc: &Procedure,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let mut callee = self.new_frame(proc);
        let mut it = args.into_iter();
        for param in &proc.params {
            if param.param_array {
                let rest: Vec<Value> = it.by_ref().collect();
                callee.set(&param.name, variant_array(rest)?);
                break;
            }
            callee.set(&param.name, it.next().unwrap_or(Value::Empty));
        }
        self.run_proc(proc, &mut callee)
    }

    /// Invokes `proc` from a call site: evaluates arguments in `caller`, binds
    /// them, runs the body, and copies `ByRef` lvalue arguments back.
    fn invoke_call(
        &mut self,
        proc: &Procedure,
        args: &[Argument],
        caller: &mut Frame,
    ) -> Result<Value, RuntimeError> {
        let (positional, named) = split_args(args);
        let mut inner = self.new_frame(proc);
        // Per-parameter argument expression actually used (for ByRef copy-out).
        let mut bound: Vec<Option<&Expr>> = vec![None; proc.params.len()];

        for (i, param) in proc.params.iter().enumerate() {
            if param.param_array {
                let mut rest = Vec::new();
                for slot in positional.iter().skip(i).flatten() {
                    rest.push(self.eval(slot, caller)?);
                }
                inner.set(&param.name, variant_array(rest)?);
                break;
            }
            let expr = named
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&param.name))
                .map(|(_, e)| *e)
                .or_else(|| positional.get(i).copied().flatten());
            bound[i] = expr;
            let value = match expr {
                Some(e) => self.eval(e, caller)?,
                None => match &param.default {
                    Some(d) => self.eval(d, caller)?,
                    None => Value::Empty,
                },
            };
            inner.set(&param.name, value);
        }

        let result = self.run_proc(proc, &mut inner)?;

        // ByRef copy-out.
        for (i, param) in proc.params.iter().enumerate() {
            if param.by_val || param.param_array {
                continue;
            }
            if let Some(expr) = bound[i]
                && is_lvalue(expr)
            {
                let final_val = inner.get(&param.name).cloned().unwrap_or(Value::Empty);
                self.assign_lvalue(expr, final_val, caller)?;
            }
        }
        Ok(result)
    }

    fn new_frame(&self, proc: &Procedure) -> Frame {
        let ret_key = proc.kind.returns_value().then(|| key(&proc.name));
        Frame::new(ret_key, self.compare_text())
    }

    /// Runs a procedure body, honouring the call-depth cap, returning its result
    /// value. A `Halt` (`End`/`Stop`) unwinds as an untrappable halt sentinel.
    fn run_proc(&mut self, proc: &Procedure, callee: &mut Frame) -> Result<Value, RuntimeError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(RuntimeError::new(28, "Out of stack space"));
        }
        self.call_depth += 1;
        let result = self.exec_body(&proc.body, callee);
        self.call_depth -= 1;
        match result? {
            Flow::Halt => Err(RuntimeError::halt()),
            _ => Ok(callee.return_value()),
        }
    }
}

/// Splits an argument list into ordered positional slots (with `None` for
/// omitted) and named `(name, expr)` pairs.
fn split_args(args: &[Argument]) -> (Vec<Option<&Expr>>, Vec<(&str, &Expr)>) {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for a in args {
        match &a.name {
            Some(n) => {
                if let Some(v) = &a.value {
                    named.push((n.as_str(), v));
                }
            }
            None => positional.push(a.value.as_ref()),
        }
    }
    (positional, named)
}

/// Builds a 0-based `Variant` array from values (for `ParamArray`).
fn variant_array(values: Vec<Value>) -> Result<Value, RuntimeError> {
    if values.is_empty() {
        return Ok(Value::Array(Array::new(vec![(0, -1)])?));
    }
    let mut arr = Array::new(vec![(0, values.len() as i32 - 1)])?;
    for (i, v) in values.into_iter().enumerate() {
        arr.set(&[i as i32], v)?;
    }
    Ok(Value::Array(arr))
}

/// Whether an argument expression is a simple assignable lvalue eligible for
/// `ByRef` copy-out (a variable or an array element).
fn is_lvalue(e: &Expr) -> bool {
    match e {
        Expr::Var(_) => true,
        Expr::Call { callee, .. } => matches!(&**callee, Expr::Var(_)),
        _ => false,
    }
}
