// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Writes an OPC package to a ZIP archive.
//!
//! Serialises `[Content_Types].xml`, the package relationships, every content
//! part with its part-level relationships, and the optional core-properties and
//! thumbnail parts. All entries are deflate-compressed.

use std::io::{Seek, Write};

use zip::write::{FileOptions, ZipWriter};

use crate::{
    constants::{MEDIA_TYPE_RELATIONSHIPS, REL_CORE_PROPERTIES, REL_THUMBNAIL},
    content_types::write_content_types,
    core_properties::write_core_properties,
    error::{OpcError, OpcResult},
    package::Package,
    part::PartName,
    relationships::{
        Relationship, TargetMode, package_relationships_part, write_relationships_part,
    },
};

/// Writes `pkg` to `writer` as an OPC-compliant ZIP archive.
///
/// `rels` and `xml` default content types are injected automatically. Core
/// properties and the thumbnail, when present, are written to
/// `/docProps/core.xml` and `/docProps/thumbnail.jpeg` and registered in the
/// package relationships. Returns [`OpcError::DigitalSignaturesNotSupported`]
/// if the package carries digital signatures, which cannot be re-emitted.
pub fn write_package_to_zip<W: Write + Seek>(pkg: &Package, writer: &mut W) -> OpcResult<()> {
    let mut zip = ZipWriter::new(writer);
    let options = FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644)
        .large_file(false); // Office packages never need ZIP64.

    let mut core_props_part_name = None;
    let mut thumb_part_name = None;

    if pkg.has_digital_signatures() {
        return Err(OpcError::DigitalSignaturesNotSupported);
    }

    let package_rels_name = package_relationships_part();
    let mut pkg_rels = pkg.relationships().clone();

    if let Some(cp) = pkg.core_properties() {
        let cp_bytes = write_core_properties(cp)?;
        let name = PartName::new("/docProps/core.xml")?;
        core_props_part_name = Some((name.clone(), cp_bytes));

        if pkg_rels.get("rIdCoreProps").is_none() {
            pkg_rels.add(Relationship {
                id: "rIdCoreProps".to_string(),
                rel_type: REL_CORE_PROPERTIES.to_string(),
                target: name.as_str()[1..].to_string(),
                target_mode: TargetMode::Internal,
            })?;
        }
    }

    if let Some(t) = pkg.thumbnail() {
        let name = PartName::new("/docProps/thumbnail.jpeg")?;
        thumb_part_name = Some((name.clone(), t.clone()));

        if pkg_rels.get("rIdThumbnail").is_none() {
            pkg_rels.add(Relationship {
                id: "rIdThumbnail".to_string(),
                rel_type: REL_THUMBNAIL.to_string(),
                target: name.as_str()[1..].to_string(),
                target_mode: TargetMode::Internal,
            })?;
        }
    }

    let mut ctm = pkg.content_type_map().clone();
    ctm.add_default("rels", MEDIA_TYPE_RELATIONSHIPS);
    ctm.add_default("xml", "application/xml");

    let ct_bytes = write_content_types(&ctm)?;
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(&ct_bytes)?;

    if !pkg_rels.is_empty() {
        let rels_bytes = write_relationships_part(&pkg_rels)?;
        let name_str = package_rels_name
            .as_str()
            .strip_prefix('/')
            .unwrap_or(package_rels_name.as_str());
        zip.start_file(name_str, options)?;
        zip.write_all(&rels_bytes)?;
    }

    for (name, data) in pkg.parts_map() {
        let name_str = name.as_str().strip_prefix('/').unwrap_or(name.as_str());
        zip.start_file(name_str, options)?;
        zip.write_all(&data.bytes)?;

        if let Some(rels) = pkg.part_relationships(name)
            && !rels.is_empty()
        {
            let r_name = name.relationships_part_name();
            let r_str = r_name.as_str().strip_prefix('/').unwrap_or(r_name.as_str());
            let rels_bytes = write_relationships_part(rels)?;
            zip.start_file(r_str, options)?;
            zip.write_all(&rels_bytes)?;
        }
    }

    if let Some((name, bytes)) = core_props_part_name {
        let name_str = name.as_str().strip_prefix('/').unwrap_or(name.as_str());
        zip.start_file(name_str, options)?;
        zip.write_all(&bytes)?;
    }

    if let Some((name, t)) = thumb_part_name {
        let name_str = name.as_str().strip_prefix('/').unwrap_or(name.as_str());
        zip.start_file(name_str, options)?;
        zip.write_all(&t.bytes)?;
    }

    zip.finish()?;
    Ok(())
}
