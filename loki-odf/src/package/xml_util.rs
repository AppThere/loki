// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Low-level XML name helpers shared within the package module.

/// Extract the local name (bytes after last `:`) from a qualified name.
pub(super) fn local_name_bytes(qname: &[u8]) -> &[u8] {
    if let Some(pos) = qname.iter().rposition(|&b| b == b':') {
        &qname[pos + 1..]
    } else {
        qname
    }
}
