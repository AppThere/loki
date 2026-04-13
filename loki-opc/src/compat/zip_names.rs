// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Handles backslash substitutions identifying non-compliant ZIP paths locally ensuring standardizations mapping files transparently supporting [MS-OI29500] / [MS-OE376].

use crate::error::DeviationWarning;

/// Replaces restricted backslash segments emitted by early configuration drivers matching native separators unconditionally correctly isolating mapping limits explicitly verifying output boundaries generating standard paths exclusively generating tracking variants globally supporting legacy parsing.
#[allow(clippy::ptr_arg)]
pub fn normalize_backslash(
    name: &str,
    #[allow(unused_variables)] warnings: &mut Vec<DeviationWarning>,
) -> String {
    #[cfg(not(feature = "strict"))]
    {
        if name.contains('\\') {
            warnings.push(DeviationWarning::BackslashInPartName {
                original: name.to_string(),
            });
            return name.replace('\\', "/");
        }
    }
    name.to_string()
}
