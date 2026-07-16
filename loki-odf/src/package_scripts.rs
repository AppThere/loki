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

/// Returns `true` if `path` lives under a script-library subtree.
fn is_script_path(path: &str) -> bool {
    SCRIPT_PREFIXES.iter().any(|p| path.starts_with(p))
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
}
