// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Re-emission of preserved ODF macro/script libraries on export (spec §3.3).
//!
//! Shared by ODT and ODS export: given a preserved [`MacroPayload`] of kind
//! [`MacroPayloadKind::OdfBasic`], this emits the manifest `<file-entry>` lines
//! and writes each script file back into the ZIP verbatim. Directory entries
//! (empty-byte parts whose path ends in `/`) contribute a manifest line only.
//! Loki does not parse or execute the scripts — this is byte preservation.

use std::io::{Seek, Write};

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::error::OdfResult;

/// Extracts the preserved ODF script payload from a document's provenance, if
/// it carries one of the right kind.
#[must_use]
pub(crate) fn odf_script_payload(macros: Option<&MacroPayload>) -> Option<&MacroPayload> {
    macros.filter(|m| m.kind == MacroPayloadKind::OdfBasic && !m.is_empty())
}

/// Builds the `<manifest:file-entry>` lines for a preserved script payload,
/// preserving each entry's declared media type and path.
#[must_use]
pub(crate) fn script_manifest_entries(payload: &MacroPayload) -> String {
    let mut m = String::new();
    for part in &payload.parts {
        // Signature files are never manifest-declared in ODF; they are written
        // to the ZIP directly (see `write_script_parts`) but not listed here.
        if crate::package::scripts::is_signature_path(&part.name) {
            continue;
        }
        let media = part.media_type.as_deref().unwrap_or("");
        m.push_str(&format!(
            "<manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{}\"/>",
            escape(&part.name),
            escape(media),
        ));
    }
    m
}

/// Writes each preserved script *file* (non-empty payload) into the ZIP.
/// Directory-only entries carry no bytes and are represented in the manifest
/// alone, so they are skipped here.
pub(crate) fn write_script_parts<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    payload: &MacroPayload,
) -> OdfResult<()> {
    let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
    for part in &payload.parts {
        if part.name.ends_with('/') || part.bytes.is_empty() {
            continue;
        }
        zip.start_file(&part.name, stored)?;
        zip.write_all(&part.bytes)?;
    }
    Ok(())
}

/// Minimal XML-attribute escaping for manifest paths/media types.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::io::macros::PreservedPart;

    fn payload() -> MacroPayload {
        MacroPayload::new(
            MacroPayloadKind::OdfBasic,
            vec![
                PreservedPart::new("Basic/", Some(String::new()), Vec::new()),
                PreservedPart::new(
                    "Basic/Standard/Module1.xml",
                    Some("text/xml".into()),
                    b"<script/>".to_vec(),
                ),
            ],
        )
    }

    #[test]
    fn manifest_lists_all_entries_including_dirs() {
        let m = script_manifest_entries(&payload());
        assert!(m.contains("full-path=\"Basic/\""));
        assert!(
            m.contains("full-path=\"Basic/Standard/Module1.xml\" manifest:media-type=\"text/xml\"")
        );
    }

    #[test]
    fn odf_script_payload_filters_wrong_kind() {
        let vba = MacroPayload::new(MacroPayloadKind::OoxmlVba, Vec::new());
        assert!(odf_script_payload(Some(&vba)).is_none());
        let basic = payload();
        assert!(odf_script_payload(Some(&basic)).is_some());
    }

    #[test]
    fn manifest_excludes_signature_files() {
        let mut p = payload();
        p.parts.push(PreservedPart::new(
            "META-INF/macrosignatures.xml",
            Some("text/xml".into()),
            b"<document-signatures/>".to_vec(),
        ));
        let m = script_manifest_entries(&p);
        // The signature file is written to the ZIP but never manifest-declared.
        assert!(!m.contains("macrosignatures"));
        assert!(m.contains("full-path=\"Basic/Standard/Module1.xml\""));
    }
}
