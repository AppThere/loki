// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`Dialect`] flag and its per-dialect semantic knobs.
//!
//! VBA (MS-VBAL) and `StarBasic` (`OpenOffice` Basic) are near-siblings: one AST
//! and one evaluator serve both, with the handful of documented divergences
//! (default `Option Base`, string/identifier case rules, dialect-only
//! built-ins) gated on this flag. See macro spec §4.2.

/// Which BASIC dialect the lexer, parser, and evaluator should follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// Microsoft Visual Basic for Applications (MS-VBAL). The default target,
    /// since the OOXML macro corpus is overwhelmingly VBA.
    #[default]
    Vba,
    /// `OpenOffice` / `LibreOffice` `StarBasic` (`OpenOffice` Basic grammar).
    StarBasic,
}

impl Dialect {
    /// The default lower bound for array dimensions when a program does not
    /// specify `Option Base`. Both dialects default to `0`; a program may
    /// override to `1` with `Option Base 1`.
    #[must_use]
    pub fn default_option_base(self) -> i32 {
        0
    }

    /// Whether integer division and `Mod` follow VBA's banker's-free truncation
    /// semantics. Both dialects truncate toward zero for `\` and take the sign
    /// of the dividend for `Mod`; kept as a hook for future divergences.
    #[must_use]
    pub fn truncating_int_division(self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_vba() {
        assert_eq!(Dialect::default(), Dialect::Vba);
    }

    #[test]
    fn default_option_base_is_zero() {
        assert_eq!(Dialect::Vba.default_option_base(), 0);
        assert_eq!(Dialect::StarBasic.default_option_base(), 0);
    }
}
