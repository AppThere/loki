// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `MsgBox` / `InputBox` — the two dialog "functions", routed to the host as
//! effects (macro spec §5.2 `UiDialog`, §5.5).
//!
//! These are **not** pure built-ins (they have an observable effect and must be
//! gated), so they are dispatched here rather than in `builtins`. The
//! interpreter evaluates the arguments, packages a [`DialogRequest`], and hands
//! it to [`Host::dialog`]; the host gates it against the `UiDialog` capability
//! and renders it in the anti-spoof frame. A denied dialog surfaces as a
//! trappable runtime error so a well-written macro degrades gracefully.

use super::Interp;
use super::env::Frame;
use crate::ast::Argument;
use crate::error::RuntimeError;
use crate::host::{DialogKind, DialogRequest, Host};
use crate::value::Value;

/// Whether `name` is a dialog function handled here (case-insensitive).
#[must_use]
pub(super) fn is_dialog(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "msgbox" | "inputbox")
}

impl<H: Host> Interp<'_, H> {
    /// Evaluates a `MsgBox`/`InputBox` call's arguments and routes it to the
    /// host as a [`DialogRequest`].
    pub(super) fn call_dialog(
        &mut self,
        name: &str,
        args: &[Argument],
        frame: &mut Frame,
    ) -> Result<Value, RuntimeError> {
        let vals = self.eval_positional(args, frame)?;
        let req = if name.eq_ignore_ascii_case("msgbox") {
            DialogRequest {
                kind: DialogKind::Message,
                prompt: opt_string(&vals, 0)?,
                buttons: opt_i64(&vals, 1)?.unwrap_or(0),
                title: opt_opt_string(&vals, 2)?,
                default: None,
            }
        } else {
            DialogRequest {
                kind: DialogKind::Input,
                prompt: opt_string(&vals, 0)?,
                buttons: 0,
                title: opt_opt_string(&vals, 1)?,
                default: opt_opt_string(&vals, 2)?,
            }
        };
        self.host_mut().dialog(&req)
    }

    /// Evaluates positional argument expressions to values (`Empty` for omitted
    /// slots). Named arguments are not supported for the dialog functions.
    fn eval_positional(
        &mut self,
        args: &[Argument],
        frame: &mut Frame,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            match &a.value {
                Some(e) => vals.push(self.eval(e, frame)?),
                None => vals.push(Value::Empty),
            }
        }
        Ok(vals)
    }
}

fn opt_string(vals: &[Value], n: usize) -> Result<String, RuntimeError> {
    match vals.get(n) {
        Some(v) => v.to_basic_string(),
        None => Ok(String::new()),
    }
}

fn opt_opt_string(vals: &[Value], n: usize) -> Result<Option<String>, RuntimeError> {
    match vals.get(n) {
        Some(Value::Empty) | None => Ok(None),
        Some(v) => Ok(Some(v.to_basic_string()?)),
    }
}

fn opt_i64(vals: &[Value], n: usize) -> Result<Option<i64>, RuntimeError> {
    match vals.get(n) {
        Some(Value::Empty) | None => Ok(None),
        Some(v) => Ok(Some(v.to_i64_round()?)),
    }
}
