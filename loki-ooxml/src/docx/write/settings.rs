// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Writer for `word/settings.xml`.
//!
//! Only the settings needed for round-trip fidelity are emitted:
//! `w:evenAndOddHeaders` (ECMA-376 §17.10.1, enables even-page
//! headers/footers) and `w:mirrorMargins` (§17.15.1.55, facing-page margin
//! mirroring — gap #27).

use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::constants::REL_SETTINGS;
use crate::docx::write::xml::NS_W;
use crate::error::OoxmlError;

/// OOXML content type for the document settings part (ECMA-376 §17.15).
const MT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";

/// Builds a minimal `word/settings.xml` carrying the requested flags.
fn write_settings_xml(even_odd: bool, mirror_margins: bool) -> Vec<u8> {
    let mut body = String::new();
    if even_odd {
        body.push_str("<w:evenAndOddHeaders/>");
    }
    if mirror_margins {
        body.push_str("<w:mirrorMargins/>");
    }
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<w:settings xmlns:w=\"{ns}\">{body}</w:settings>"
        ),
        ns = NS_W,
        body = body,
    )
    .into_bytes()
}

/// Adds `word/settings.xml` to `pkg`: the part, its content-type override, and
/// a document relationship (using the caller-supplied `r_id`).
///
/// Call only when the document needs at least one flag; without this part
/// even-header references and margin mirroring are ignored by consumers.
pub(super) fn wire_settings(
    pkg: &mut Package,
    doc_part: &PartName,
    r_id: String,
    even_odd: bool,
    mirror_margins: bool,
) -> Result<(), OoxmlError> {
    let settings_part = PartName::new("/word/settings.xml").map_err(OoxmlError::Opc)?;
    pkg.set_part(
        settings_part.clone(),
        PartData::new(write_settings_xml(even_odd, mirror_margins), MT_SETTINGS),
    );
    pkg.part_relationships_mut(doc_part)
        .add(Relationship {
            id: r_id,
            rel_type: REL_SETTINGS.to_string(),
            target: "settings.xml".to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;
    pkg.content_type_map_mut()
        .add_override(&settings_part, MT_SETTINGS);
    Ok(())
}
