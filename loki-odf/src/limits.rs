// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Hard limits guarding ODF import against hostile inputs.
//!
//! ODF packages are attacker-controlled: a tiny file can declare huge
//! decompressed entries (zip bomb), huge `number-rows-repeated` /
//! `number-columns-repeated` counts (allocation amplification), or deeply
//! nested elements (stack exhaustion). The constants here bound each of
//! those axes; [`read_entry_capped`] enforces the decompression budgets.

use std::io::Read;

use crate::error::{OdfError, OdfResult};

/// Maximum decompressed size of a single ZIP entry: 256 MiB.
///
/// Exceeding it yields [`OdfError::EntryTooLarge`] instead of exhausting
/// memory on a zip bomb.
pub(crate) const MAX_ENTRY_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum aggregate decompressed size of all entries in a package: 1 GiB.
///
/// Exceeding it yields [`OdfError::PackageTooLarge`].
pub(crate) const MAX_PACKAGE_DECOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;

/// Maximum repeat count that is *materialized* as individual cells.
///
/// Real-world ODS files use huge repeats (e.g. `number-rows-repeated=
/// "1048576"`) to describe trailing empty rows; those must not drive
/// per-cell insert loops. Cells carrying content or a style are
/// materialized at most this many times per axis, while row/column
/// indices still advance by the full (sheet-clamped) repeat count so
/// subsequent cells land in the right place.
pub(crate) const MAX_MATERIALIZED_REPEAT: u32 = 10_000;

/// Whole-workbook ceiling on the number of cells an import will
/// materialize. The per-axis [`MAX_MATERIALIZED_REPEAT`] alone still
/// allows a single cell with both `number-rows-repeated` and
/// `number-columns-repeated` near the cap to expand to ~10⁸ cells, and a
/// sheet can contain many such cells; this aggregate budget bounds the
/// total regardless. Once exceeded, further cells advance the cursor but
/// are not inserted.
pub(crate) const MAX_MATERIALIZED_CELLS_TOTAL: u64 = 2_000_000;

/// Maximum row index space of an ODS sheet (matches the common
/// 1,048,576-row spreadsheet limit). Repeat counts that advance the row
/// cursor are clamped to this.
pub(crate) const MAX_SHEET_ROWS: u32 = 1_048_576;

/// Maximum column index space of an ODS sheet (matches the common
/// 16,384-column spreadsheet limit). Repeat counts that advance the
/// column cursor are clamped to this.
pub(crate) const MAX_SHEET_COLS: u32 = 16_384;

/// Maximum number of column specs expanded from a single
/// `table:table-column` `number-columns-repeated` attribute in ODT tables.
pub(crate) const MAX_TABLE_COLUMNS: u32 = 16_384;

/// Maximum space count honoured for a single `<text:s text:c="N"/>`
/// element; larger counts are clamped instead of allocating N bytes.
pub(crate) const MAX_REPEATED_SPACES: u32 = 10_000;

/// Maximum element nesting depth for recursive ODT structures
/// (nested `text:span` / `text:a`, nested `text:list`). Exceeding it
/// yields [`OdfError::NestingTooDeep`] instead of overflowing the stack.
pub(crate) const MAX_NESTING_DEPTH: usize = 100;

/// Read a ZIP entry to a `Vec<u8>` enforcing the default decompression
/// budgets.
///
/// `total_decompressed` accumulates bytes read across the whole package;
/// the caller threads one counter through all entry reads. The reader is
/// wrapped with `take(limit + 1)` so at most one byte past the budget is
/// ever pulled — declared sizes are never used to pre-allocate.
pub(crate) fn read_entry_capped(
    reader: impl Read,
    name: &str,
    total_decompressed: &mut u64,
) -> OdfResult<Vec<u8>> {
    read_entry_capped_with(
        reader,
        name,
        MAX_ENTRY_DECOMPRESSED_BYTES,
        MAX_PACKAGE_DECOMPRESSED_BYTES,
        total_decompressed,
    )
}

/// Read an entry enforcing explicit budgets (separated out for testability).
fn read_entry_capped_with(
    reader: impl Read,
    name: &str,
    entry_limit: u64,
    package_limit: u64,
    total_decompressed: &mut u64,
) -> OdfResult<Vec<u8>> {
    let mut data = Vec::new();
    reader
        .take(entry_limit.saturating_add(1))
        .read_to_end(&mut data)?;
    if data.len() as u64 > entry_limit {
        return Err(OdfError::EntryTooLarge {
            name: name.to_string(),
            limit: entry_limit,
        });
    }
    *total_decompressed = total_decompressed.saturating_add(data.len() as u64);
    if *total_decompressed > package_limit {
        return Err(OdfError::PackageTooLarge {
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
        let data = read_entry_capped_with(payload.as_slice(), "content.xml", 64, 1024, &mut total)
            .expect("read should succeed");
        assert_eq!(data.len(), 64);
        assert_eq!(total, 64);
    }

    #[test]
    fn entry_over_budget_is_rejected() {
        let payload = [0u8; 65];
        let mut total = 0u64;
        let result =
            read_entry_capped_with(payload.as_slice(), "content.xml", 64, 1024, &mut total);
        assert!(
            matches!(result, Err(OdfError::EntryTooLarge { ref name, limit: 64 }) if name == "content.xml"),
            "expected EntryTooLarge, got {result:?}"
        );
    }

    #[test]
    fn aggregate_over_budget_is_rejected() {
        let payload = [0u8; 50];
        let mut total = 0u64;
        // Two entries of 50 bytes each against an 80-byte package budget.
        read_entry_capped_with(payload.as_slice(), "content.xml", 64, 80, &mut total)
            .expect("first entry fits");
        let result = read_entry_capped_with(payload.as_slice(), "styles.xml", 64, 80, &mut total);
        assert!(
            matches!(result, Err(OdfError::PackageTooLarge { limit: 80 })),
            "expected PackageTooLarge, got {result:?}"
        );
    }
}
