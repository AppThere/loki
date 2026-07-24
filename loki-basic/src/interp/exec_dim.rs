// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `Dim`/`ReDim` execution and local-variable default construction. Split from
//! [`super::exec`] for the 300-line ceiling.

use super::Interp;
use super::env::Frame;
use crate::ast::VarDecl;
use crate::error::RuntimeError;
use crate::host::Host;
use crate::value::{Array, Value};

impl<H: Host> Interp<'_, H> {
    pub(super) fn exec_dim(
        &mut self,
        decls: &[VarDecl],
        frame: &mut Frame,
    ) -> Result<(), RuntimeError> {
        for d in decls {
            let v = self.local_default(d, frame)?;
            frame.set(&d.name, v);
        }
        Ok(())
    }

    pub(super) fn exec_redim(
        &mut self,
        preserve: bool,
        decls: &[VarDecl],
        frame: &mut Frame,
    ) -> Result<(), RuntimeError> {
        for d in decls {
            let fresh = self.local_default(d, frame)?;
            let value = if preserve {
                preserve_into(frame.get(&d.name), fresh)
            } else {
                fresh
            };
            frame.set(&d.name, value);
        }
        Ok(())
    }

    /// The initial value for a local declaration (array bounds evaluated in the
    /// current frame, so `Dim a(n)` works).
    fn local_default(&mut self, decl: &VarDecl, frame: &mut Frame) -> Result<Value, RuntimeError> {
        let Some(bounds) = &decl.bounds else {
            return Ok(super::typed_default(&decl.ty));
        };
        if bounds.is_empty() {
            return Ok(Value::Array(Array::new(Vec::new())?));
        }
        let mut dims = Vec::with_capacity(bounds.len());
        for b in bounds {
            let lo = match &b.lower {
                Some(e) => self.eval(e, frame)?.to_i32()?,
                None => self.option_base(),
            };
            let hi = self.eval(&b.upper, frame)?.to_i32()?;
            dims.push((lo, hi));
        }
        Ok(Value::Array(Array::new(dims)?))
    }
}

/// `ReDim Preserve`: copies overlapping 1-D elements from the old array into the
/// fresh one. Multi-dimensional preserve re-creates without copying (Phase 2
/// simplification).
fn preserve_into(old: Option<&Value>, fresh: Value) -> Value {
    let (Some(Value::Array(old)), Value::Array(mut new)) = (old, fresh.clone()) else {
        return fresh;
    };
    if old.rank() != 1 || new.rank() != 1 {
        return Value::Array(new);
    }
    let (Some(lo), Some(hi_old), Some(hi_new)) = (new.lbound(1), old.ubound(1), new.ubound(1))
    else {
        return Value::Array(new);
    };
    let hi = hi_old.min(hi_new);
    let mut i = lo;
    while i <= hi {
        if let Ok(v) = old.get(&[i]) {
            let _ = new.set(&[i], v);
        }
        i += 1;
    }
    Value::Array(new)
}
