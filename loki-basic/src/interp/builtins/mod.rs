// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The pure-compute standard library.
//!
//! Built-ins are **pure functions over already-evaluated argument values** —
//! they have no host access and cannot perform I/O (macro spec §4.2: anything
//! effectful is a host capability or refused, never a built-in). The
//! interpreter evaluates the argument expressions, then dispatches here.

mod convert;
mod info;
mod math;
mod strings;

use crate::ast::Argument;
use crate::error::RuntimeError;
use crate::host::Host;
use crate::interp::Interp;
use crate::interp::env::Frame;
use crate::value::Value;

/// Every recognised built-in name (lowercased). Membership gates dispatch so an
/// unknown name is reported as "Sub or Function not defined" rather than
/// silently returning `Empty`.
const NAMES: &[&str] = &[
    // math
    "abs",
    "sgn",
    "int",
    "fix",
    "sqr",
    "sin",
    "cos",
    "tan",
    "atn",
    "exp",
    "log",
    "round",
    // conversion
    "cint",
    "clng",
    "cdbl",
    "cbool",
    "cstr",
    "val",
    "str",
    "hex",
    "oct",
    // strings
    "len",
    "left",
    "right",
    "mid",
    "ucase",
    "lcase",
    "trim",
    "ltrim",
    "rtrim",
    "space",
    "string",
    "chr",
    "asc",
    "instr",
    "instrrev",
    "replace",
    "strreverse",
    // arrays / info
    "lbound",
    "ubound",
    "isnumeric",
    "isempty",
    "isnull",
    "isarray",
    "isobject",
    "typename",
    "iif",
];

/// Whether `name` is a recognised built-in.
#[must_use]
pub(crate) fn is_builtin(name: &str) -> bool {
    NAMES.contains(&name.to_ascii_lowercase().as_str())
}

impl<H: Host> Interp<'_, H> {
    /// Evaluates a built-in's argument expressions and dispatches to the pure
    /// implementation.
    pub(super) fn call_builtin(
        &mut self,
        name: &str,
        args: &[Argument],
        frame: &mut Frame,
    ) -> Result<Value, RuntimeError> {
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            match &a.value {
                Some(e) => vals.push(self.eval(e, frame)?),
                None => vals.push(Value::Empty), // omitted / Missing
            }
        }
        dispatch(&name.to_ascii_lowercase(), &vals)
    }
}

fn dispatch(name: &str, a: &[Value]) -> Result<Value, RuntimeError> {
    match name {
        "abs" | "sgn" | "int" | "fix" | "sqr" | "sin" | "cos" | "tan" | "atn" | "exp" | "log"
        | "round" => math::call(name, a),
        "cint" | "clng" | "cdbl" | "cbool" | "cstr" | "val" | "str" | "hex" | "oct" => {
            convert::call(name, a)
        }
        "len" | "left" | "right" | "mid" | "ucase" | "lcase" | "trim" | "ltrim" | "rtrim"
        | "space" | "string" | "chr" | "asc" | "instr" | "instrrev" | "replace" | "strreverse" => {
            strings::call(name, a)
        }
        "lbound" | "ubound" | "isnumeric" | "isempty" | "isnull" | "isarray" | "isobject"
        | "typename" | "iif" => info::call(name, a),
        _ => Err(RuntimeError::new(35, "Sub or Function not defined")),
    }
}

/// The `n`th argument, or `Empty` if absent.
pub(super) fn arg(a: &[Value], n: usize) -> Value {
    a.get(n).cloned().unwrap_or(Value::Empty)
}

/// Requires exactly (or at least) `min` arguments, else "invalid procedure call".
pub(super) fn require(a: &[Value], min: usize) -> Result<(), RuntimeError> {
    if a.len() >= min {
        Ok(())
    } else {
        Err(RuntimeError::invalid_call())
    }
}

/// Parses a `m/d/yyyy` date to an OLE automation serial (used by date literals;
/// full date built-ins land in Phase 13).
#[must_use]
pub(crate) fn parse_us_date(s: &str) -> Option<f64> {
    let mut parts = s.split('/');
    let m: i64 = parts.next()?.trim().parse().ok()?;
    let d: i64 = parts.next()?.trim().parse().ok()?;
    let y: i64 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(serial_from_ymd(y, m, d))
}

/// Days from 1899-12-30 (the OLE automation epoch) to `y-m-d`.
fn serial_from_ymd(y: i64, m: i64, d: i64) -> f64 {
    // Convert to a Julian day number, then offset to the OLE epoch.
    let a = (14 - m) / 12;
    let yy = y + 4800 - a;
    let mm = m + 12 * a - 3;
    let jdn = d + (153 * mm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045;
    // JDN of 1899-12-30 is 2415018.
    (jdn - 2_415_019) as f64
}
