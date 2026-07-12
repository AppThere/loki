// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Explicit conditional-format membership for one table cell — OOXML
//! `w:cnfStyle` (ECMA-376 §17.4.7), 4a.3.
//!
//! Word stamps each cell (and row) with a 12-flag binary mask naming exactly
//! which table-style regions apply, so consumers need not re-derive banding
//! positionally. When present it is authoritative: it survives row
//! re-ordering and unusual band sizes that positional derivation would get
//! wrong. Carried on the cell's `NodeAttr` under the `"cnf"` key (the same
//! opaque-code convention as the table's `"tbllook"`), so it round-trips
//! through the Loro CRDT for free.

use super::table_style::TableRegion;

/// The decoded `w:cnfStyle` mask: which conditional regions this cell claims.
///
/// Field order matches the ECMA-376 §17.4.7 bit string:
/// `firstRow lastRow firstColumn lastColumn band1Vert band2Vert band1Horz
/// band2Horz neCell nwCell seCell swCell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableCnf {
    /// In the header row (`firstRow`).
    pub first_row: bool,
    /// In the total/footer row (`lastRow`).
    pub last_row: bool,
    /// In the leading column (`firstColumn`).
    pub first_column: bool,
    /// In the trailing column (`lastColumn`).
    pub last_column: bool,
    /// In an odd vertical band (`band1Vert`).
    pub band1_vert: bool,
    /// In an even vertical band (`band2Vert`).
    pub band2_vert: bool,
    /// In an odd horizontal band (`band1Horz`).
    pub band1_horz: bool,
    /// In an even horizontal band (`band2Horz`).
    pub band2_horz: bool,
    /// The top-right corner cell (`neCell`).
    pub ne_cell: bool,
    /// The top-left corner cell (`nwCell`).
    pub nw_cell: bool,
    /// The bottom-right corner cell (`seCell`).
    pub se_cell: bool,
    /// The bottom-left corner cell (`swCell`).
    pub sw_cell: bool,
}

impl TableCnf {
    /// Decodes the 12-character `0`/`1` mask (the `w:cnfStyle @w:val` form,
    /// stored verbatim in the cell attr). `None` for any other shape.
    #[must_use]
    pub fn decode_attr(code: &str) -> Option<Self> {
        if code.len() != 12 || !code.bytes().all(|b| b == b'0' || b == b'1') {
            return None;
        }
        let bit = |i: usize| code.as_bytes()[i] == b'1';
        Some(Self {
            first_row: bit(0),
            last_row: bit(1),
            first_column: bit(2),
            last_column: bit(3),
            band1_vert: bit(4),
            band2_vert: bit(5),
            band1_horz: bit(6),
            band2_horz: bit(7),
            ne_cell: bit(8),
            nw_cell: bit(9),
            se_cell: bit(10),
            sw_cell: bit(11),
        })
    }

    /// Encodes back to the 12-character mask.
    #[must_use]
    pub fn encode_attr(&self) -> String {
        [
            self.first_row,
            self.last_row,
            self.first_column,
            self.last_column,
            self.band1_vert,
            self.band2_vert,
            self.band1_horz,
            self.band2_horz,
            self.ne_cell,
            self.nw_cell,
            self.se_cell,
            self.sw_cell,
        ]
        .iter()
        .map(|&b| if b { '1' } else { '0' })
        .collect()
    }

    /// Whether this mask claims membership of `region`.
    /// [`TableRegion::WholeTable`] always applies.
    #[must_use]
    pub fn contains(&self, region: TableRegion) -> bool {
        match region {
            TableRegion::WholeTable => true,
            TableRegion::FirstRow => self.first_row,
            TableRegion::LastRow => self.last_row,
            TableRegion::FirstColumn => self.first_column,
            TableRegion::LastColumn => self.last_column,
            TableRegion::Band1Horz => self.band1_horz,
            TableRegion::Band2Horz => self.band2_horz,
            TableRegion::Band1Vert => self.band1_vert,
            TableRegion::Band2Vert => self.band2_vert,
            TableRegion::NwCell => self.nw_cell,
            TableRegion::NeCell => self.ne_cell,
            TableRegion::SwCell => self.sw_cell,
            TableRegion::SeCell => self.se_cell,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_round_trips_and_maps_regions() {
        let code = "100100001000"; // firstRow + lastColumn + neCell
        let cnf = TableCnf::decode_attr(code).expect("valid mask");
        assert!(cnf.first_row && cnf.last_column && cnf.ne_cell);
        assert!(!cnf.last_row && !cnf.band1_horz);
        assert!(cnf.contains(TableRegion::FirstRow));
        assert!(cnf.contains(TableRegion::NeCell));
        assert!(cnf.contains(TableRegion::WholeTable), "always applies");
        assert!(!cnf.contains(TableRegion::Band1Vert));
        assert_eq!(cnf.encode_attr(), code);
    }

    #[test]
    fn malformed_masks_are_rejected() {
        assert!(TableCnf::decode_attr("").is_none());
        assert!(TableCnf::decode_attr("10010000100").is_none()); // 11 chars
        assert!(TableCnf::decode_attr("10010000100x").is_none());
    }
}
