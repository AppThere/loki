// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! VBA macro-payload preservation for OOXML (spec §3, Phase 1).
//!
//! Shared by DOCX (`.docm`/`.dotm`, main part `word/document.xml`) and XLSX
//! (`.xlsm`/`.xltm`, main part `xl/workbook.xml`): in both, the VBA project is
//! reached from the main part via the Microsoft `vbaProject` relationship.
//!
//! Loki does **not** execute VBA in Phase 1. It preserves the payload
//! byte-for-byte so that opening a macro-enabled document and saving it back
//! to a macro-enabled extension no longer silently destroys the macros (the
//! pre-Phase-1 behaviour: the importer walked only known relationship types
//! and the exporter built a fresh package, so `vbaProject.bin` vanished).
//!
//! Scope: the `vbaProject.bin` VBA project and its optional `vbaData.xml`
//! companion (Word only). The project bytes are preserved verbatim; the OPC
//! wiring (relationships, content-type overrides) is **regenerated** on export
//! to its standard, well-known values — semantically identical to the
//! original, and robust against unusual rId assignments. Loki never parses the
//! CFB container or executes p-code (that is Phase 3 / `loki-vba`).

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::error::OoxmlError;

// ── Microsoft VBA relationship types (MS-DOCX / MS-OSHARED) ─────────────────

/// Relationship from the main document part to the VBA project.
pub(crate) const REL_VBA_PROJECT: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
/// Relationship from the VBA project to its `vbaData.xml` companion.
pub(crate) const REL_WORD_VBA_DATA: &str =
    "http://schemas.microsoft.com/office/2006/relationships/wordVbaData";

// ── VBA content types ───────────────────────────────────────────────────────

const CT_VBA_PROJECT: &str = "application/vnd.ms-office.vbaProject";
const CT_VBA_DATA: &str = "application/vnd.ms-word.vbaData+xml";

/// Standard suffixes used to classify preserved parts on export.
const SUFFIX_VBA_PROJECT: &str = "vbaProject.bin";
const SUFFIX_VBA_DATA: &str = "vbaData.xml";

/// Collects the VBA payload from an imported package, if present.
///
/// Follows the `vbaProject` relationship from the main document part, then
/// the `wordVbaData` relationship from the project part. Returns `None` when
/// the document declares no VBA project.
pub(crate) fn collect(package: &Package, doc_part: &PartName) -> Option<MacroPayload> {
    let doc_rels = package.part_relationships(doc_part)?;
    let vba_rel = doc_rels.iter().find(|r| r.rel_type == REL_VBA_PROJECT)?;

    let project_name = resolve(doc_part, &vba_rel.target)?;
    let project = package.part(&project_name)?;

    let mut parts = vec![PreservedPart::new(
        project_name.as_str(),
        package
            .content_type(&project_name)
            .map(str::to_owned)
            .or_else(|| Some(CT_VBA_PROJECT.to_owned())),
        project.bytes.clone(),
    )];

    // Optional vbaData.xml, referenced from the project part's own rels.
    if let Some(proj_rels) = package.part_relationships(&project_name)
        && let Some(data_rel) = proj_rels.iter().find(|r| r.rel_type == REL_WORD_VBA_DATA)
        && let Some(data_name) = resolve(&project_name, &data_rel.target)
        && let Some(data) = package.part(&data_name)
    {
        parts.push(PreservedPart::new(
            data_name.as_str(),
            package
                .content_type(&data_name)
                .map(str::to_owned)
                .or_else(|| Some(CT_VBA_DATA.to_owned())),
            data.bytes.clone(),
        ));
    }

    Some(MacroPayload::new(MacroPayloadKind::OoxmlVba, parts))
}

/// Re-emits a preserved VBA payload into a freshly-assembled package.
///
/// Inserts the preserved parts verbatim, declares their content-type
/// overrides, and regenerates the document→project (and project→data)
/// relationships with a fresh, collision-free relationship id.
///
/// No-op for a non-VBA payload (defensive; DOCX assembly only calls this for
/// `OoxmlVba`).
pub(crate) fn emit(
    pkg: &mut Package,
    doc_part: &PartName,
    payload: &MacroPayload,
    next_rel_id: impl FnMut() -> String,
) -> Result<(), OoxmlError> {
    if payload.kind != MacroPayloadKind::OoxmlVba {
        return Ok(());
    }

    let mut next_rel_id = next_rel_id;

    // Insert every preserved part and declare its content type.
    let mut project_name: Option<PartName> = None;
    let mut data_name: Option<PartName> = None;
    for part in &payload.parts {
        let name = PartName::new(part.name.clone()).map_err(OoxmlError::Opc)?;
        let media = part
            .media_type
            .clone()
            .unwrap_or_else(|| default_media_type(&part.name).to_owned());
        pkg.content_type_map_mut().add_override(&name, &media);
        pkg.set_part(name.clone(), PartData::new(part.bytes.clone(), &media));

        if part.name.ends_with(SUFFIX_VBA_PROJECT) {
            project_name = Some(name);
        } else if part.name.ends_with(SUFFIX_VBA_DATA) {
            data_name = Some(name);
        }
    }

    let Some(project_name) = project_name else {
        // A payload without a project part is malformed; parts are still
        // preserved above, but there is nothing to wire a relationship to.
        return Ok(());
    };

    // document.xml → vbaProject.bin
    let project_target = relative_target(doc_part, &project_name);
    pkg.part_relationships_mut(doc_part)
        .add(Relationship {
            id: next_rel_id(),
            rel_type: REL_VBA_PROJECT.to_string(),
            target: project_target,
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;

    // vbaProject.bin → vbaData.xml
    if let Some(data_name) = data_name {
        let data_target = relative_target(&project_name, &data_name);
        pkg.part_relationships_mut(&project_name)
            .add(Relationship {
                id: "rId1".to_string(),
                rel_type: REL_WORD_VBA_DATA.to_string(),
                target: data_target,
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    Ok(())
}

fn default_media_type(part_name: &str) -> &'static str {
    if part_name.ends_with(SUFFIX_VBA_DATA) {
        CT_VBA_DATA
    } else {
        CT_VBA_PROJECT
    }
}

/// Resolves a relationship `target` (relative or absolute) against `base`.
fn resolve(base: &PartName, target: &str) -> Option<PartName> {
    if target.starts_with('/') {
        return PartName::new(target).ok();
    }
    let base = base.as_str();
    let dir = base.rfind('/').map_or("/", |i| &base[..=i]);
    PartName::new(format!("{dir}{target}")).ok()
}

/// Computes the target path of `to` relative to the directory of `from`.
///
/// Both are sibling parts under the same directory in every real document
/// (`/word/…`), so this reduces to the file name; if they diverge, the
/// absolute path is used (always valid in an OPC `.rels`).
fn relative_target(from: &PartName, to: &PartName) -> String {
    let from = from.as_str();
    let dir = from.rfind('/').map_or("/", |i| &from[..=i]);
    to.as_str()
        .strip_prefix(dir)
        .map_or_else(|| to.as_str().to_owned(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_against_word_dir() {
        let base = PartName::new("/word/document.xml").unwrap();
        let got = resolve(&base, "vbaProject.bin").unwrap();
        assert_eq!(got.as_str(), "/word/vbaProject.bin");
    }

    #[test]
    fn resolve_absolute_target() {
        let base = PartName::new("/word/document.xml").unwrap();
        let got = resolve(&base, "/word/vbaProject.bin").unwrap();
        assert_eq!(got.as_str(), "/word/vbaProject.bin");
    }

    #[test]
    fn relative_target_is_sibling_file_name() {
        let from = PartName::new("/word/document.xml").unwrap();
        let to = PartName::new("/word/vbaProject.bin").unwrap();
        assert_eq!(relative_target(&from, &to), "vbaProject.bin");
    }

    #[test]
    fn default_media_types() {
        assert_eq!(default_media_type("/word/vbaProject.bin"), CT_VBA_PROJECT);
        assert_eq!(default_media_type("/word/vbaData.xml"), CT_VBA_DATA);
    }
}
