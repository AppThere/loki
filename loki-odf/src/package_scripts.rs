// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `StarBasic` / script-library preservation for ODF packages (spec §3, Phase 1).
//!
//! ODF stores macros as a `Basic/` library subtree (`StarBasic`) and/or a
//! `Scripts/` subtree (other script providers), each file declared in
//! `META-INF/manifest.xml`. Loki does **not** execute these in Phase 1; it
//! preserves them byte-for-byte so a load→edit→save cycle no longer silently
//! strips them (the pre-Phase-1 reader extracted only a fixed part list).
//!
//! Collection is **manifest-driven** so each preserved entry keeps its exact
//! declared media type and so directory entries (which have no ZIP payload)
//! round-trip too. File bytes come from the ZIP. The `<office:scripts>` event
//! bindings inside `content.xml` are a separate concern (that part is
//! regenerated on export); binding-level round-trip is deferred to the
//! execution phases.

use std::io::{Read, Seek};

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

use crate::error::OdfResult;
use crate::limits::read_entry_capped;

/// Path prefixes that hold macro/script libraries in an ODF package.
const SCRIPT_PREFIXES: [&str; 2] = ["Basic/", "Scripts/"];

/// The W3C-XMLDSig macro/document signature files. These are **not** declared in
/// `manifest.xml` (ODF excludes the signature files from the manifest), so they
/// are collected out-of-band and, on export, kept out of the manifest too. Only
/// `macrosignatures.xml` is the macro-trust anchor (`documentsignatures.xml` is
/// display-only), but both are preserved for round-trip fidelity.
const SIGNATURE_PARTS: [&str; 2] = [
    "META-INF/macrosignatures.xml",
    "META-INF/documentsignatures.xml",
];

/// Returns `true` if `path` lives under a script-library subtree.
fn is_script_path(path: &str) -> bool {
    SCRIPT_PREFIXES.iter().any(|p| path.starts_with(p))
}

/// Returns `true` if `path` is an ODF signature file (kept out of the manifest).
#[must_use]
pub(crate) fn is_signature_path(path: &str) -> bool {
    SIGNATURE_PARTS.contains(&path)
}

/// Collects the ODF script payload, if any, driven by the manifest.
///
/// `manifest` is the raw `META-INF/manifest.xml` bytes (already read by the
/// package opener). Returns `None` when the package declares no script
/// libraries.
pub(super) fn collect_scripts<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    manifest: &[u8],
    total_decompressed: &mut u64,
) -> OdfResult<Option<MacroPayload>> {
    let declared = parse_manifest_scripts(manifest);
    if declared.is_empty() {
        return Ok(None);
    }

    let mut parts = Vec::with_capacity(declared.len());
    for (path, media_type) in declared {
        if path.ends_with('/') {
            // Directory entry: manifest-only, no ZIP payload.
            parts.push(PreservedPart::new(path, Some(media_type), Vec::new()));
            continue;
        }
        // File entry: read its bytes verbatim from the ZIP.
        if let Ok(mut entry) = archive.by_name(&path) {
            let bytes = read_entry_capped(&mut entry, &path, total_decompressed)?;
            parts.push(PreservedPart::new(path, Some(media_type), bytes));
        }
        // A manifest entry with no matching ZIP file is malformed input; skip it
        // rather than fail — the rest of the payload is still worth preserving.
    }

    if parts.iter().all(|p| p.bytes.is_empty()) {
        // Only directory entries and no actual script files: nothing to keep.
        return Ok(None);
    }

    // Preserve any macro/document signature files (not manifest-declared) so the
    // signature can be verified on open (8A.8) and round-trips on save.
    for sig_path in SIGNATURE_PARTS {
        if let Ok(mut entry) = archive.by_name(sig_path) {
            let bytes = read_entry_capped(&mut entry, sig_path, total_decompressed)?;
            parts.push(PreservedPart::new(
                sig_path,
                Some("text/xml".to_owned()),
                bytes,
            ));
        }
    }

    Ok(Some(MacroPayload::new(MacroPayloadKind::OdfBasic, parts)))
}

/// Parses `manifest` and returns `(full_path, media_type)` for every
/// `<manifest:file-entry>` under a script subtree, preserving declaration
/// order.
fn parse_manifest_scripts(manifest: &[u8]) -> Vec<(String, String)> {
    let mut reader = Reader::from_reader(manifest);
    reader.config_mut().trim_text(false);

    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                if local(e.local_name().into_inner()) == b"file-entry" {
                    let path = attr(e, b"full-path");
                    if let Some(path) = path.filter(|p| is_script_path(p)) {
                        let media = attr(e, b"media-type").unwrap_or_default();
                        out.push((path, media));
                    }
                }
                buf.clear();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => buf.clear(),
        }
    }
    out
}

/// Reads an attribute's value by local name (namespace-prefix-insensitive).
fn attr(e: &quick_xml::events::BytesStart<'_>, local_name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local(a.key.local_name().into_inner()) == local_name {
            String::from_utf8(a.value.into_owned()).ok()
        } else {
            None
        }
    })
}

/// Local part (after the last `:`) of a qualified name.
fn local(qname: &[u8]) -> &[u8] {
    qname
        .iter()
        .rposition(|&b| b == b':')
        .map_or(qname, |pos| &qname[pos + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &[u8] = br#"<?xml version="1.0"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0">
<manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/>
<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="Basic/" manifest:media-type=""/>
<manifest:file-entry manifest:full-path="Basic/Standard/" manifest:media-type=""/>
<manifest:file-entry manifest:full-path="Basic/Standard/Module1.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="Basic/script-lc.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#;

    #[test]
    fn parses_only_script_entries() {
        let got = parse_manifest_scripts(MANIFEST);
        let paths: Vec<&str> = got.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "Basic/",
                "Basic/Standard/",
                "Basic/Standard/Module1.xml",
                "Basic/script-lc.xml",
            ]
        );
        // content.xml and the root entry are excluded.
        assert!(!paths.contains(&"content.xml"));
    }

    #[test]
    fn is_script_path_matches_both_subtrees() {
        assert!(is_script_path("Basic/Standard/Module1.xml"));
        assert!(is_script_path("Scripts/python/foo.py"));
        assert!(!is_script_path("Pictures/img.png"));
    }

    #[test]
    fn is_signature_path_matches_the_signature_files_only() {
        assert!(is_signature_path("META-INF/macrosignatures.xml"));
        assert!(is_signature_path("META-INF/documentsignatures.xml"));
        assert!(!is_signature_path("Basic/Standard/Module1.xml"));
        assert!(!is_signature_path("META-INF/manifest.xml"));
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::FileOptions::<()>::default();
            for (name, bytes) in entries {
                w.start_file(*name, opts).unwrap();
                w.write_all(bytes).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    #[test]
    fn preserves_macrosignatures_alongside_scripts() {
        let module = b"<module>Sub AutoOpen()\nEnd Sub</module>";
        let sig = b"<document-signatures>signed</document-signatures>";
        let zip = zip_with(&[
            ("Basic/Standard/Module1.xml", module),
            ("META-INF/macrosignatures.xml", sig),
        ]);
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
        let mut total = 0u64;
        let payload = collect_scripts(&mut archive, MANIFEST, &mut total)
            .unwrap()
            .expect("payload");

        let sig_part = payload
            .parts
            .iter()
            .find(|p| p.name == "META-INF/macrosignatures.xml")
            .expect("signature preserved");
        assert_eq!(sig_part.bytes, sig);
        // The signature is the last part, after the declared script entries.
        assert_eq!(
            payload.parts.last().map(|p| p.name.as_str()),
            Some("META-INF/macrosignatures.xml")
        );
    }

    #[test]
    fn no_signature_part_when_absent() {
        let module = b"<module/>";
        let zip = zip_with(&[("Basic/Standard/Module1.xml", module)]);
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
        let mut total = 0u64;
        let payload = collect_scripts(&mut archive, MANIFEST, &mut total)
            .unwrap()
            .expect("payload");
        assert!(!payload.parts.iter().any(|p| is_signature_path(&p.name)));
    }
}
