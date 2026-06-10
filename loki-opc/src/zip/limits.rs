// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Decompression budgets guarding package reads against ZIP bombs.
//!
//! A hostile archive can declare tiny compressed entries that inflate to
//! gigabytes. All entry reads in [`crate::zip::read`] go through
//! [`read_entry_capped`], which enforces a per-entry budget and an aggregate
//! per-package budget while reading, without trusting any declared sizes.

use std::io::Read;

use crate::error::{OpcError, OpcResult};

/// Maximum decompressed size of a single ZIP entry: 256 MiB.
///
/// Reads stop at `limit + 1` bytes; an entry that would exceed the budget
/// yields [`OpcError::EntryTooLarge`] instead of exhausting memory.
pub(crate) const MAX_ENTRY_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum aggregate decompressed size of all entries in a package: 1 GiB.
///
/// Exceeding the aggregate budget yields [`OpcError::PackageTooLarge`].
pub(crate) const MAX_PACKAGE_DECOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;

/// Read an entry to a `Vec<u8>` enforcing the default budgets.
///
/// `total_decompressed` accumulates the bytes read so far for the whole
/// package; the caller threads one counter through all entry reads.
pub(crate) fn read_entry_capped(
    reader: impl Read,
    name: &str,
    total_decompressed: &mut u64,
) -> OpcResult<Vec<u8>> {
    read_entry_capped_with(
        reader,
        name,
        MAX_ENTRY_DECOMPRESSED_BYTES,
        MAX_PACKAGE_DECOMPRESSED_BYTES,
        total_decompressed,
    )
}

/// Read an entry enforcing explicit budgets (separated out for testability).
///
/// The reader is wrapped with `take(entry_limit + 1)` so at most one byte
/// past the budget is ever pulled; declared sizes are never used to
/// pre-allocate.
fn read_entry_capped_with(
    reader: impl Read,
    name: &str,
    entry_limit: u64,
    package_limit: u64,
    total_decompressed: &mut u64,
) -> OpcResult<Vec<u8>> {
    let mut data = Vec::new();
    reader
        .take(entry_limit.saturating_add(1))
        .read_to_end(&mut data)?;
    if data.len() as u64 > entry_limit {
        return Err(OpcError::EntryTooLarge {
            name: name.to_string(),
            limit: entry_limit,
        });
    }
    *total_decompressed = total_decompressed.saturating_add(data.len() as u64);
    if *total_decompressed > package_limit {
        return Err(OpcError::PackageTooLarge {
            limit: package_limit,
        });
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_within_budget_is_read_fully() {
        let payload = [7u8; 64];
        let mut total = 0u64;
        let data = read_entry_capped_with(payload.as_slice(), "/part.xml", 64, 1024, &mut total)
            .expect("read should succeed");
        assert_eq!(data.len(), 64);
        assert_eq!(total, 64);
    }

    #[test]
    fn entry_over_budget_is_rejected() {
        let payload = [0u8; 65];
        let mut total = 0u64;
        let result = read_entry_capped_with(payload.as_slice(), "/bomb.xml", 64, 1024, &mut total);
        assert!(
            matches!(result, Err(OpcError::EntryTooLarge { ref name, limit: 64 }) if name == "/bomb.xml"),
            "expected EntryTooLarge, got {result:?}"
        );
    }

    #[test]
    fn aggregate_over_budget_is_rejected() {
        let payload = [0u8; 50];
        let mut total = 0u64;
        // Two entries of 50 bytes each against a 80-byte package budget.
        read_entry_capped_with(payload.as_slice(), "/a.xml", 64, 80, &mut total)
            .expect("first entry fits");
        let result = read_entry_capped_with(payload.as_slice(), "/b.xml", 64, 80, &mut total);
        assert!(
            matches!(result, Err(OpcError::PackageTooLarge { limit: 80 })),
            "expected PackageTooLarge, got {result:?}"
        );
    }
}
