// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Statement execution: the label/`GoTo`/error-handling body loop, the
//! per-statement dispatch, assignment, and `Dim`/`ReDim`. Compound statements
//! live in [`super::exec_block`].

use std::collections::HashMap;

use super::env::{ErrState, ErrorHandler, Frame, key};
use super::{Flow, Interp};
use crate::ast::{ExitKind, Expr, OnError, ResumeKind, Stmt};
use crate::error::RuntimeError;
use crate::host::Host;
use crate::value::Value;

impl<H: Host> Interp<'_, H> {
    /// Executes a procedure body (top-level statement list) with label
    /// resolution, `On Error` trapping, and `Resume`.
    pub(super) fn exec_body(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        let labels = label_map(stmts);
        let mut pc = 0;
        while pc < stmts.len() {
            match self.exec_stmt(&stmts[pc], frame) {
                Ok(Flow::Normal) => pc += 1,
                Ok(Flow::Goto(l)) => pc = jump(&labels, &l)?,
                Ok(Flow::ResumeRetry) => {
                    frame.err.clear();
                    pc = frame.resume_pc.unwrap_or(pc);
                }
                Ok(Flow::ResumeNext) => {
                    frame.err.clear();
                    pc = frame.resume_pc.map_or(pc + 1, |p| p + 1);
                }
                Ok(Flow::ResumeLabel(l)) => {
                    frame.err.clear();
                    pc = jump(&labels, &l)?;
                }
                Ok(other) => return Ok(other),
                Err(e) if e.is_halt() || !e.trappable => return Err(e),
                Err(e) => pc = trap(e, pc, &labels, frame)?,
            }
        }
        Ok(Flow::Normal)
    }

    /// Executes a single statement, returning its control-flow outcome.
    pub(super) fn exec_stmt(
        &mut self,
        stmt: &Stmt,
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        self.step()?;
        match stmt {
            Stmt::Empty | Stmt::Label(_) => Ok(Flow::Normal),
            Stmt::Dim(decls) => self.exec_dim(decls, frame).map(|()| Flow::Normal),
            Stmt::ReDim { preserve, decls } => self
                .exec_redim(*preserve, decls, frame)
                .map(|()| Flow::Normal),
            Stmt::Const(decls) => {
                for d in decls {
                    let v = self.eval(&d.value, frame)?;
                    frame.set(&d.name, v);
                }
                Ok(Flow::Normal)
            }
            Stmt::Assign { target, value } | Stmt::Set { target, value } => {
                let v = self.eval(value, frame)?;
                self.assign_lvalue(target, v, frame)?;
                Ok(Flow::Normal)
            }
            Stmt::Call(expr) => self.exec_call_stmt(expr, frame).map(|()| Flow::Normal),
            Stmt::If { .. } => self.exec_if(stmt, frame),
            Stmt::For { .. } | Stmt::ForEach { .. } => self.exec_for(stmt, frame),
            Stmt::DoLoop { .. } => self.exec_do(stmt, frame),
            Stmt::While { .. } => self.exec_while(stmt, frame),
            Stmt::SelectCase { .. } => self.exec_select(stmt, frame),
            Stmt::With { .. } => self.exec_with(stmt, frame),
            Stmt::Exit(kind) => Ok(exit_flow(*kind)),
            Stmt::GoTo(l) => Ok(Flow::Goto(l.clone())),
            Stmt::OnError(oe) => {
                frame.handler = match oe {
                    OnError::GoToLabel(l) => ErrorHandler::Label(l.clone()),
                    OnError::Disable => ErrorHandler::None,
                    OnError::ResumeNext => ErrorHandler::ResumeNext,
                };
                Ok(Flow::Normal)
            }
            Stmt::Resume(kind) => Ok(match kind {
                ResumeKind::Retry => Flow::ResumeRetry,
                ResumeKind::Next => Flow::ResumeNext,
                ResumeKind::Label(l) => Flow::ResumeLabel(l.clone()),
            }),
            Stmt::ErrorStmt(e) => {
                let n = self.eval(e, frame)?.to_i32()?;
                Err(RuntimeError::new(n, "Application-defined error"))
            }
            Stmt::Halt => Ok(Flow::Halt),
        }
    }

    /// Runs the compound-statement bodies (sequential; no label scope).
    pub(super) fn exec_block(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        for stmt in stmts {
            match self.exec_stmt(stmt, frame)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    /// Assigns `value` to an lvalue (variable or array element).
    pub(super) fn assign_lvalue(
        &mut self,
        target: &Expr,
        value: Value,
        frame: &mut Frame,
    ) -> Result<(), RuntimeError> {
        match target {
            Expr::Var(name) => {
                if frame.has(name) {
                    frame.set(name, value);
                } else if let Some(slot) = self.globals.get_mut(&key(name)) {
                    *slot = value;
                } else {
                    frame.set(name, value);
                }
                Ok(())
            }
            // `obj.Prop = value` — property assignment on a host object.
            Expr::Member { object, name } => {
                let recv = self.eval_receiver(object, frame)?;
                self.host_mut().set_member(recv, name, value)
            }
            Expr::Call { callee, args } => {
                // `obj.Prop(i) = value` — an indexed/default property put is not
                // supported in v1 (Phase 6); fall through to error below.
                let Expr::Var(name) = &**callee else {
                    return Err(RuntimeError::new(424, "Object required"));
                };
                let indices = self.eval_indices(args, frame)?;
                if let Some(Value::Array(arr)) = frame.get_mut(name) {
                    return arr.set(&indices, value);
                }
                if let Some(Value::Array(arr)) = self.globals.get_mut(&key(name)) {
                    return arr.set(&indices, value);
                }
                Err(RuntimeError::type_mismatch())
            }
            _ => Err(RuntimeError::new(424, "Object required")),
        }
    }

    fn exec_call_stmt(&mut self, expr: &Expr, frame: &mut Frame) -> Result<(), RuntimeError> {
        // Debug.Print / Debug.Assert: evaluate args for effect, produce no output.
        if let Expr::Call { callee, args } = expr {
            if let Expr::Member { object, .. } = &**callee {
                if matches!(&**object, Expr::Var(o) if o.eq_ignore_ascii_case("Debug")) {
                    for a in args {
                        if let Some(e) = &a.value {
                            self.eval(e, frame)?;
                        }
                    }
                    return Ok(());
                }
                // `obj.Method arg1, arg2` as a statement — dispatch to the host
                // (its result, if any, is discarded).
                self.eval(expr, frame)?;
                return Ok(());
            }
            self.eval(expr, frame)?;
            return Ok(());
        }
        // A bare `obj.Method` statement with no args (member call, no parens).
        if let Expr::Member { object, name } = expr {
            // `Err.Clear` (no parens) is a method, not a property read.
            if self.is_err_receiver(object, frame) {
                super::eval::call_err_method(name, &[], frame)?;
                return Ok(());
            }
            self.eval(expr, frame)?;
            return Ok(());
        }
        // Bare name: call a Sub/Function/builtin if it names one, else evaluate.
        if let Expr::Var(name) = expr {
            if let Some(&proc) = self.procs.get(&key(name)) {
                self.invoke_with_values(proc, Vec::new())?;
                return Ok(());
            }
            if super::builtins::is_builtin(name) {
                self.call_builtin(name, &[], frame)?;
                return Ok(());
            }
        }
        self.eval(expr, frame)?;
        Ok(())
    }
}

fn exit_flow(kind: ExitKind) -> Flow {
    match kind {
        ExitKind::For => Flow::ExitFor,
        ExitKind::Do => Flow::ExitDo,
        ExitKind::Sub | ExitKind::Function | ExitKind::Property => Flow::ExitProc,
    }
}

/// Resolves a label name to its statement index, or a "label not defined"
/// runtime error.
fn jump(labels: &HashMap<String, usize>, label: &str) -> Result<usize, RuntimeError> {
    labels
        .get(&key(label))
        .copied()
        .ok_or_else(|| RuntimeError::new(0, format!("Label not defined: {label}")))
}

/// Handles a trapped runtime error per the frame's active handler, returning the
/// next program counter (or re-raising if there is no handler).
fn trap(
    e: RuntimeError,
    pc: usize,
    labels: &HashMap<String, usize>,
    frame: &mut Frame,
) -> Result<usize, RuntimeError> {
    frame.resume_pc = Some(pc);
    frame.err = ErrState {
        number: e.number,
        description: e.message.clone(),
    };
    match frame.handler.clone() {
        ErrorHandler::None => Err(e),
        ErrorHandler::ResumeNext => Ok(pc + 1),
        ErrorHandler::Label(l) => jump(labels, &l),
    }
}

/// Builds the label → statement-index map for a body.
fn label_map(stmts: &[Stmt]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (i, s) in stmts.iter().enumerate() {
        if let Stmt::Label(name) = s {
            map.insert(key(name), i);
        }
    }
    map
}
