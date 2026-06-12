// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Internal helper functions for OPC part resolution and relationship lookup.

use loki_opc::{PartName, RelationshipSet};

use crate::error::OoxmlError;

pub(crate) type OpcResult<T> = Result<T, OoxmlError>;

/// Wraps an [`OoxmlError::Xml`] with the given part path for context.
pub(crate) fn map_xml_err(e: OoxmlError, _part: &str) -> OoxmlError {
    // The error already carries its part context from the reader; pass through.
    e
}

/// Resolves a target path relative to a base part name into a [`PartName`].
///
/// `base` should be a valid OPC part name (e.g. `"/word/document.xml"`).
/// If `target` starts with `/`, it is used as-is. Otherwise, the parent
/// directory of `base` is prepended.
pub(crate) fn resolve_part_name(base: &str, target: &str) -> OpcResult<PartName> {
    if target.starts_with('/') {
        return PartName::new(target).map_err(OoxmlError::Opc);
    }
    let dir = base.rfind('/').map_or("/", |i| &base[..=i]);
    PartName::new(format!("{dir}{target}")).map_err(OoxmlError::Opc)
}

/// Helper to retrieve relationships by type supporting both transitional and strict namespaces.
pub(crate) fn rels_by_type<'a>(
    rels: &'a RelationshipSet,
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

/// Resolves an optional related part by relationship type and parses it.
///
/// Returns `None` if the relationship is not present; returns an error only
/// if the part exists but fails to parse.
pub(crate) fn resolve_optional_part<T, F>(
    package: &loki_opc::Package,
    doc_rels: Option<&RelationshipSet>,
    rel_type: &str,
    base_part: &str,
    parse: F,
) -> OpcResult<Option<T>>
where
    F: Fn(&[u8], &str) -> OpcResult<T>,
{
    let Some(rels) = doc_rels else {
        return Ok(None);
    };
    let Some(rel) = rels_by_type(rels, rel_type).next() else {
        return Ok(None);
    };
    let part_name = resolve_part_name(base_part, &rel.target)?;
    let Some(part) = package.part(&part_name) else {
        return Ok(None);
    };
    let result = parse(&part.bytes, part_name.as_str())?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_path() {
        let name = resolve_part_name("/word/document.xml", "styles.xml").unwrap();
        assert_eq!(name.as_str(), "/word/styles.xml");
    }

    #[test]
    fn resolve_absolute_path() {
        let name = resolve_part_name("/word/document.xml", "/word/styles.xml").unwrap();
        assert_eq!(name.as_str(), "/word/styles.xml");
    }
}
