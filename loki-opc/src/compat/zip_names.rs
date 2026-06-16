// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! ZIP path compatibility shim: rewrite backslash separators emitted by
//! non-conformant writers to forward slashes ([MS-OI29500] / [MS-OE376]).

use crate::error::DeviationWarning;

/// Rewrites backslashes in a ZIP entry `name` to forward slashes.
///
/// Outside the `strict` feature, names containing a backslash are normalised and
/// a [`DeviationWarning::BackslashInPartName`] is recorded; conformant names are
/// returned unchanged.
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
