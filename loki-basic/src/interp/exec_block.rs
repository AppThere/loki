// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Compound-statement execution: `If`, `For`/`For Each`, `Do`/`While`,
//! `Select Case`, and `With`.

use super::env::Frame;
use super::{Flow, Interp};
use crate::ast::{BinOp, CaseCond, CompareOp, DoCond, Stmt};
use crate::error::RuntimeError;
use crate::host::Host;
use crate::value::{Value, binary_op};

impl<H: Host> Interp<'_, H> {
    pub(super) fn exec_if(&mut self, stmt: &Stmt, frame: &mut Frame) -> Result<Flow, RuntimeError> {
        let Stmt::If {
            branches,
            else_body,
        } = stmt
        else {
            unreachable!()
        };
        for (cond, body) in branches {
            if self.eval(cond, frame)?.to_bool()? {
                return self.exec_block(body, frame);
            }
        }
        match else_body {
            Some(eb) => self.exec_block(eb, frame),
            None => Ok(Flow::Normal),
        }
    }

    pub(super) fn exec_for(
        &mut self,
        stmt: &Stmt,
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        match stmt {
            Stmt::For {
                var,
                from,
                to,
                step,
                body,
            } => {
                let mut cur = self.eval(from, frame)?.to_f64()?;
                let end = self.eval(to, frame)?.to_f64()?;
                let step = match step {
                    Some(e) => self.eval(e, frame)?.to_f64()?,
                    None => 1.0,
                };
                loop {
                    self.step()?;
                    if (step >= 0.0 && cur > end) || (step < 0.0 && cur < end) {
                        break;
                    }
                    frame.set(var, counter_value(cur));
                    match self.exec_block(body, frame)? {
                        Flow::Normal => {}
                        Flow::ExitFor => break,
                        other => return Ok(other),
                    }
                    // Re-read the counter (the body may have modified it).
                    if let Some(v) = frame.get(var) {
                        cur = v.to_f64()?;
                    }
                    cur += step;
                }
                Ok(Flow::Normal)
            }
            Stmt::ForEach {
                var,
                collection,
                body,
            } => {
                let coll = self.eval(collection, frame)?;
                let Value::Array(arr) = coll else {
                    return Err(RuntimeError::type_mismatch());
                };
                for v in arr.values().to_vec() {
                    self.step()?;
                    frame.set(var, v);
                    match self.exec_block(body, frame)? {
                        Flow::Normal => {}
                        Flow::ExitFor => break,
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn exec_do(&mut self, stmt: &Stmt, frame: &mut Frame) -> Result<Flow, RuntimeError> {
        let Stmt::DoLoop { pre, post, body } = stmt else {
            unreachable!()
        };
        loop {
            self.step()?;
            if self.do_stop(pre.as_ref(), frame)? {
                break;
            }
            match self.exec_block(body, frame)? {
                Flow::Normal => {}
                Flow::ExitDo => break,
                other => return Ok(other),
            }
            if self.do_stop(post.as_ref(), frame)? {
                break;
            }
        }
        Ok(Flow::Normal)
    }

    /// Whether a `While`/`Until` guard says to stop looping now.
    fn do_stop(&mut self, cond: Option<&DoCond>, frame: &mut Frame) -> Result<bool, RuntimeError> {
        Ok(match cond {
            Some(DoCond::While(e)) => !self.eval(e, frame)?.to_bool()?,
            Some(DoCond::Until(e)) => self.eval(e, frame)?.to_bool()?,
            None => false,
        })
    }

    pub(super) fn exec_while(
        &mut self,
        stmt: &Stmt,
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        let Stmt::While { cond, body } = stmt else {
            unreachable!()
        };
        loop {
            self.step()?;
            if !self.eval(cond, frame)?.to_bool()? {
                break;
            }
            match self.exec_block(body, frame)? {
                Flow::Normal => {}
                Flow::ExitDo => break,
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    pub(super) fn exec_select(
        &mut self,
        stmt: &Stmt,
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        let Stmt::SelectCase {
            subject,
            cases,
            else_body,
        } = stmt
        else {
            unreachable!()
        };
        let subj = self.eval(subject, frame)?;
        for case in cases {
            for cond in &case.conditions {
                if self.case_matches(&subj, cond, frame)? {
                    return self.exec_block(&case.body, frame);
                }
            }
        }
        match else_body {
            Some(eb) => self.exec_block(eb, frame),
            None => Ok(Flow::Normal),
        }
    }

    fn case_matches(
        &mut self,
        subject: &Value,
        cond: &CaseCond,
        frame: &mut Frame,
    ) -> Result<bool, RuntimeError> {
        let ct = self.compare_text();
        match cond {
            CaseCond::Value(e) => {
                let v = self.eval(e, frame)?;
                binary_op(BinOp::Eq, subject, &v, ct)?.to_bool()
            }
            CaseCond::Range(lo, hi) => {
                let lo = self.eval(lo, frame)?;
                let hi = self.eval(hi, frame)?;
                let ge = binary_op(BinOp::Ge, subject, &lo, ct)?.to_bool()?;
                let le = binary_op(BinOp::Le, subject, &hi, ct)?.to_bool()?;
                Ok(ge && le)
            }
            CaseCond::Compare(op, e) => {
                let v = self.eval(e, frame)?;
                binary_op(compare_binop(*op), subject, &v, ct)?.to_bool()
            }
        }
    }

    pub(super) fn exec_with(
        &mut self,
        stmt: &Stmt,
        frame: &mut Frame,
    ) -> Result<Flow, RuntimeError> {
        let Stmt::With { object, body } = stmt else {
            unreachable!()
        };
        let obj = self.eval(object, frame)?;
        frame.with_stack.push(obj);
        let result = self.exec_block(body, frame);
        frame.with_stack.pop();
        result
    }
}

/// Chooses the `Value` type for a `For` counter: an integer when the current
/// value is integral and fits `Long`, else a `Double`.
fn counter_value(cur: f64) -> Value {
    if cur.fract() == 0.0 && cur.abs() < 2_147_483_648.0 {
        Value::from_i64_fit(cur as i64)
    } else {
        Value::Double(cur)
    }
}

fn compare_binop(op: CompareOp) -> BinOp {
    match op {
        CompareOp::Eq => BinOp::Eq,
        CompareOp::Ne => BinOp::Ne,
        CompareOp::Lt => BinOp::Lt,
        CompareOp::Le => BinOp::Le,
        CompareOp::Gt => BinOp::Gt,
        CompareOp::Ge => BinOp::Ge,
    }
}
