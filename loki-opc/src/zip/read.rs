// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Reads an OPC package from a ZIP archive.
//!
//! Each entry is decompressed under the per-entry and aggregate byte budgets in
//! the `zip::limits` budgets (zip-bomb guard), normalised through the
//! [`crate::compat`] shims (backslash separators, non-canonical percent
//! encoding), and routed by suffix: `.rels` parts become relationship sets,
//! everything else becomes a [`PartData`] keyed by [`PartName`]. UTF-16 XML
//! parts are transcoded to UTF-8 on the fly.

use std::collections::HashMap;
use std::io::{Read, Seek};

use zip::ZipArchive;

use crate::{
    constants::{REL_CORE_PROPERTIES, REL_THUMBNAIL},
    content_types::parse_content_types,
    core_properties::parse_core_properties,
    error::{OpcError, OpcResult},
    package::Package,
    part::{PartData, PartName},
    relationships::{package_relationships_part, parse_relationships_part},
    zip::limits::read_entry_capped,
};

/// Reads an OPC [`Package`] from a seekable ZIP `reader`.
///
/// Resolves `[Content_Types].xml`, every content part, package- and part-level
/// relationships, core properties, and the thumbnail. Returns
/// [`OpcError::MissingContentTypes`] when the archive has no content-types part
/// and [`OpcError::UnsupportedCompression`] for entries that are neither stored
/// nor deflated. Decompression is bounded by the `zip::limits` budgets.
pub fn read_package_from_zip<R: Read + Seek>(reader: &mut R) -> OpcResult<Package> {
    let mut zip = ZipArchive::new(reader).map_err(OpcError::Zip)?;
    let mut pkg = Package::new();

    let ct_index = crate::compat::content_types::find_content_types(&mut zip, &mut pkg.warnings)
        .ok_or(OpcError::MissingContentTypes)?;

    // Aggregate decompressed-byte budget for the whole package (zip-bomb guard).
    let mut total_decompressed: u64 = 0;

    let mut ct_xml = {
        let mut ct_file = zip.by_index(ct_index)?;
        let ct_name = ct_file.name().to_string();
        read_entry_capped(&mut ct_file, &ct_name, &mut total_decompressed)?
    };
    if let Some(transcoded) = transcode_utf16_to_utf8(&ct_xml) {
        ct_xml = transcoded;
    }

    let ctm = parse_content_types(&ct_xml, &mut pkg.warnings)?;
    *pkg.content_type_map_mut() = ctm.clone();

    let mut parts_map: HashMap<PartName, PartData> = HashMap::new();
    let mut relationships_map = HashMap::new();

    let package_rels_name = package_relationships_part();

    let mut seen_names = Vec::new();

    for i in 0..zip.len() {
        if i == ct_index {
            continue;
        }

        let mut file = zip.by_index(i)?;
        if file.is_dir() {
            continue;
        }

        let method = file.compression();
        if method != zip::CompressionMethod::Stored && method != zip::CompressionMethod::Deflated {
            return Err(OpcError::UnsupportedCompression(format!("{:?}", method)));
        }

        let mut name_str = file.name().to_string();
        name_str = crate::compat::zip_names::normalize_backslash(&name_str, &mut pkg.warnings);

        if !name_str.starts_with('/') {
            name_str.insert(0, '/');
        }

        let name =
            crate::compat::part_names::normalize_percent_encoding(&name_str, &mut pkg.warnings)?;

        seen_names.push(name.as_str().to_string());

        let mut data = read_entry_capped(&mut file, name.as_str(), &mut total_decompressed)?;
        if name.as_str().ends_with(".xml") || name.as_str().ends_with(".rels") {
            data = transcode_utf16_to_utf8(&data).unwrap_or(data);
        }

        let media_type = ctm.resolve(&name).unwrap_or("").to_string();
        if media_type.is_empty() {
            return Err(OpcError::UnknownMediaType {
                part: name.as_str().to_string(),
                extension: name.extension().unwrap_or("").to_string(),
            });
        }

        if name.as_str().ends_with(".rels") {
            let rs = parse_relationships_part(&data, name.as_str(), &mut pkg.warnings)?;
            let mut rel_set = crate::relationships::RelationshipSet::default();
            for r in rs {
                // Duplicate relationship ids within a part are dropped by
                // `add`; the first occurrence wins.
                let _ = rel_set.add(r);
            }

            if name == package_rels_name {
                *pkg.relationships_mut() = rel_set;
            } else {
                let (dir, file) = match name.as_str().rsplit_once("/_rels/") {
                    // `name` ends with ".rels" (guarded above), so `f` does too;
                    // `unwrap_or` keeps the gate panic-free regardless.
                    Some((d, f)) => (
                        d.to_string(),
                        f.strip_suffix(".rels").unwrap_or(f).to_string(),
                    ),
                    None => continue,
                };

                let target_name = PartName::new(format!("{}/{}", dir, file))?;
                relationships_map.insert(target_name, rel_set);
            }
        } else {
            if name.as_str().starts_with("/_xmlsignatures/") || name.as_str().ends_with(".sigs") {
                pkg.set_has_digital_signatures(true);
            }
            parts_map.insert(
                name,
                PartData {
                    bytes: data,
                    media_type,
                    growth_hint: 0,
                },
            );
        }
    }

    // Now core properties and thumbnails
    let cp_rel = pkg
        .relationships()
        .iter()
        .find(|r| r.rel_type == REL_CORE_PROPERTIES)
        .cloned();
    if let Some(cp_rel) = cp_rel {
        let cp_target = if cp_rel.target.starts_with('/') {
            cp_rel.target.clone()
        } else {
            format!("/{}", cp_rel.target)
        };
        if let Ok(target) = PartName::new(&cp_target)
            && let Some(data) = parts_map.remove(&target)
        {
            let cp = parse_core_properties(&data.bytes)?;
            *pkg.core_properties_mut() = cp;
        }
    }

    let t_rel = pkg
        .relationships()
        .iter()
        .find(|r| r.rel_type == REL_THUMBNAIL)
        .cloned();
    if let Some(t_rel) = t_rel {
        let t_target = if t_rel.target.starts_with('/') {
            t_rel.target.clone()
        } else {
            format!("/{}", t_rel.target)
        };
        if let Ok(target) = PartName::new(&t_target)
            && let Some(data) = parts_map.remove(&target)
        {
            pkg.set_thumbnail(data.clone(), &data.media_type);
        }
    }

    for (name, data) in parts_map {
        pkg.set_part(name, data);
    }

    for (name, rels) in relationships_map {
        *pkg.part_relationships_mut(&name) = rels;
    }

    Ok(pkg)
}

/// Transcode a UTF-16 (BE or LE) XML buffer to UTF-8 on the fly.
fn transcode_utf16_to_utf8(buf: &[u8]) -> Option<Vec<u8>> {
    if buf.len() < 2 {
        return None;
    }
    let big_endian = match (buf[0], buf[1]) {
        (0xFE, 0xFF) => true,
        (0xFF, 0xFE) => false,
        _ => return None,
    };

    let u16_data: Vec<u16> = if big_endian {
        buf[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect()
    } else {
        buf[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    };

    let string = String::from_utf16_lossy(&u16_data);
    Some(string.into_bytes())
}
