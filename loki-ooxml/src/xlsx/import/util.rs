// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Part-name resolution and relationship helpers.

use crate::error::OoxmlError;
use loki_opc::PartName;

pub(super) fn resolve_part_name(base: &str, target: &str) -> Result<PartName, OoxmlError> {
    if target.starts_with('/') {
        return PartName::new(target).map_err(OoxmlError::Opc);
    }
    let dir = base.rfind('/').map_or("/", |i| &base[..=i]);
    PartName::new(format!("{dir}{target}")).map_err(OoxmlError::Opc)
}

pub(super) fn rels_by_type<'a>(
    rels: &'a loki_opc::RelationshipSet,
    transitional_type: &str,
) -> impl Iterator<Item = &'a loki_opc::Relationship> {
    let strict_type = transitional_type.replace(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/",
        "http://purl.oclc.org/ooxml/officeDocument/relationships/",
    );
    let strict_owned = strict_type;
    let trans_owned = transitional_type.to_owned();
    rels.iter()
        .filter(move |r| r.rel_type == trans_owned || r.rel_type == strict_owned)
}

pub(super) fn cell_ref_to_coord(cell_ref: &str) -> Option<(u32, u32)> {
    let mut chars = cell_ref.chars().peekable();
    let mut col_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            col_str.push(c.to_ascii_uppercase());
            chars.next();
        } else {
            break;
        }
    }
    let row_str: String = chars.collect();
    if col_str.is_empty() || row_str.is_empty() {
        return None;
    }
    let row = row_str.parse::<u32>().ok()?.checked_sub(1)?;

    let mut col: u32 = 0;
    for c in col_str.chars() {
        col = col
            .checked_mul(26)?
            .checked_add((c as u32) - ('A' as u32) + 1)?;
    }
    col = col.checked_sub(1)?;
    Some((row, col))
}
