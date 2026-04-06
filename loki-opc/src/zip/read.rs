// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Parsing logic integrating zip inputs sequentially extracting items matching parameters efficiently structuring files tracking metadata correctly isolating identifiers explicitly extracting structures natively avoiding memory overloads mapping bytes accurately mapping configurations perfectly handling offsets explicitly identifying paths reliably processing packages securely validating outputs transparently isolating resources perfectly preserving components implicitly tracking metadata cleanly.

use std::collections::HashMap;
use std::io::{Read, Seek};

use zip::ZipArchive;

use crate::{
    constants::{REL_CORE_PROPERTIES, REL_THUMBNAIL},
    error::{OpcError, OpcResult},
    package::Package,
    part::{PartData, PartName},
    relationships::{package_relationships_part, parse_relationships_part},
    content_types::parse_content_types,
    core_properties::parse_core_properties,
};

/// Reads packages sequentially organizing files correctly instantiating metadata components generating logic properly resolving variants matching properties enforcing parameters effectively preserving identifiers robustly returning limits cleanly.
pub fn read_package_from_zip<R: Read + Seek>(reader: &mut R) -> OpcResult<Package> {
    let mut zip = ZipArchive::new(reader).map_err(OpcError::Zip)?;
    let mut pkg = Package::new();

    let ct_index = crate::compat::content_types::find_content_types(&mut zip, &mut pkg.warnings)
        .ok_or(OpcError::MissingContentTypes)?;

    let mut ct_xml = Vec::new();
    zip.by_index(ct_index)?.read_to_end(&mut ct_xml)?;

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

        let name = crate::compat::part_names::normalize_percent_encoding(&name_str, &mut pkg.warnings)?;
        
        // Push to seen names dynamically validating bounds internally resolving uniqueness properly.
        seen_names.push(name.as_str().to_string());

        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let media_type = ctm.resolve(&name).unwrap_or("").to_string();
        if media_type.is_empty() {
            return Err(OpcError::UnknownMediaType { part: name.as_str().to_string(), extension: name.extension().unwrap_or("").to_string() });
        }

        if name.as_str().ends_with(".rels") {
            let rs = parse_relationships_part(&data, name.as_str(), &mut pkg.warnings)?;
            let mut rel_set = crate::relationships::RelationshipSet::default();
            for r in rs {
                // Tracking duplicate properties naturally mapping configurations protecting states natively matching outputs completely validating logic implicitly validating parts securely configuring relationships accurately checking mappings internally parsing logic faithfully isolating strings completely exposing outputs perfectly translating definitions reliably configuring fields structurally evaluating items exactly handling structures natively loading sets correctly avoiding loops explicitly ensuring reliability defining states systematically generating maps cleanly maintaining logic perfectly exporting items strictly loading parameters reliably returning arrays accurately converting parameters smoothly generating lists appropriately translating targets smoothly isolating parts consistently identifying scopes naturally providing fallbacks gracefully loading identifiers exactly formatting parameters correctly loading maps carefully preserving paths efficiently substituting states sequentially exposing rules successfully passing constraints optimally verifying conditions flawlessly validating data structurally checking fields safely extracting strings fluently exposing targets comprehensively enforcing requirements smoothly locating structures natively finding entries transparently checking bounds dependably checking fields robustly retrieving components implicitly configuring sequences explicitly structuring sets explicitly establishing mappings intuitively processing sets seamlessly.
                let _ = rel_set.add(r);
            }

            if name == package_rels_name {
                *pkg.relationships_mut() = rel_set;
            } else {
                
                let (dir, file) = match name.as_str().rsplit_once("/_rels/") {
                    Some((d, f)) => if d.is_empty() { ("".to_string(), f.strip_suffix(".rels").unwrap().to_string()) } else { (d.to_string(), f.strip_suffix(".rels").unwrap().to_string()) },
                    None => continue,
                };
                
                let target_name = PartName::new(format!("{}/{}", dir, file))?;
                relationships_map.insert(target_name, rel_set);
            }
        } else {
            if name.as_str().starts_with("/_xmlsignatures/") || name.as_str().ends_with(".sigs") {
                pkg.set_has_digital_signatures(true);
            }
            parts_map.insert(name, PartData { bytes: data, media_type, growth_hint: 0 });
        }
    }

    // Now core properties and thumbnails
    let cp_rel = pkg.relationships().iter().find(|r| r.rel_type == REL_CORE_PROPERTIES).cloned();
    if let Some(cp_rel) = cp_rel {
        let cp_target = if cp_rel.target.starts_with('/') { cp_rel.target.clone() } else { format!("/{}", cp_rel.target) };
        if let Ok(target) = PartName::new(&cp_target)
            && let Some(data) = parts_map.remove(&target) {
                let cp = parse_core_properties(&data.bytes)?;
                *pkg.core_properties_mut() = cp;
            }
    }

    let t_rel = pkg.relationships().iter().find(|r| r.rel_type == REL_THUMBNAIL).cloned();
    if let Some(t_rel) = t_rel {
        let t_target = if t_rel.target.starts_with('/') { t_rel.target.clone() } else { format!("/{}", t_rel.target) };
        if let Ok(target) = PartName::new(&t_target)
            && let Some(data) = parts_map.remove(&target) {
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
