// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The parser's nesting-depth guard.
//!
//! The parser is recursive descent, so nesting depth in the *source* becomes
//! depth on the *native* stack — and macros are untrusted input. Without a cap,
//! a module containing a few thousand `(` (or a few thousand nested `If`s)
//! exhausts the thread stack and **aborts the host process**: a stack overflow
//! is not a catchable Rust error, so no amount of `Result` plumbing recovers
//! from it. The cap turns that abort into an ordinary [`BasicError::Parse`].
//!
//! The counter is a *stack budget* rather than a level count, because the
//! constructs differ by ~4× in what one level costs (see the constants below).
//! Charging each in proportion to its measured cost bounds the total stack no
//! matter how the shapes are mixed, while still letting the cheap ones nest
//! deeper than the expensive ones.

use super::Parser;
use crate::error::BasicError;

/// Native stack the parser may consume on nesting, in KiB.
///
/// Sized against the **smallest** stack the parser can plausibly run on: the
/// 2 MiB Rust gives a spawned thread (a test thread, or any host worker), in an
/// unoptimised build. 768 KiB leaves ~2.5× headroom over the measured overflow
/// point, and is generous against real macros, which nest ~10 levels.
pub(super) const MAX_PARSE_STACK_KIB: usize = 768;

/// Measured native-stack cost of one level of **statement** nesting
/// (`parse_block` → `parse_statement` → `parse_if` → `parse_block`).
///
/// Empirical, 2026-08, debug build on a 2 MiB thread: nested block `If`s parsed
/// at depth 75 and overflowed by depth 100 ⇒ ~24 KiB per level. Single-line
/// `If` chains measured slightly cheaper (~19 KiB) and are charged the same.
/// Budget ⇒ **32** levels of statement nesting.
pub(super) const STMT_STACK_KIB: usize = 24;

/// Measured native-stack cost of one level of **expression** nesting (a paren
/// group, a call-argument list, or a unary operator).
///
/// Same measurement run: `(`-nesting and `f(`-nesting parsed at depth 350 and
/// overflowed by 375 ⇒ ~5.6 KiB per level (`Not` chains are cheaper still, at
/// ~2 KiB, and are charged the same). Budget ⇒ **128** levels of expression
/// nesting.
pub(super) const EXPR_STACK_KIB: usize = 6;

impl Parser {
    /// Runs `f` one nesting level deeper, refusing once the budget is spent.
    ///
    /// `cost_kib` is the caller's share of [`MAX_PARSE_STACK_KIB`]. The charge
    /// is released on the way out (including the error path), so the counter
    /// tracks the *current* nesting rather than the total ever entered. Taking
    /// the body as a closure keeps charge and release paired — there is no way
    /// to enter a level and forget to leave it.
    pub(super) fn nested<T>(
        &mut self,
        cost_kib: usize,
        f: impl FnOnce(&mut Self) -> Result<T, BasicError>,
    ) -> Result<T, BasicError> {
        if self.depth + cost_kib > MAX_PARSE_STACK_KIB {
            return Err(self.error(&format!(
                "expression or block nested too deeply \
                 (parser stack budget of {MAX_PARSE_STACK_KIB} KiB exhausted)"
            )));
        }
        self.depth += cost_kib;
        let out = f(self);
        self.depth -= cost_kib;
        out
    }
}
