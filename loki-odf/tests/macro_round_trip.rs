// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Phase 1 macro-preservation round-trip for ODF (spec §3).
//!
//! Builds a synthetic ODT carrying a StarBasic library (`Basic/`), imports it,
//! re-exports it, and asserts the Basic module survives byte-for-byte. Loki
//! never parses or executes the script — this is preservation only.

use std::io::{Cursor, Write};

use loki_doc_model::io::macros::MacroPayloadKind;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_odf::{OdtExport, OdtImport};
use zip::CompressionMethod;
use zip::write::{FileOptions, ZipWriter};

const MODULE1: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<script:module xmlns:script="urn:oasis:names:tc:opendocument:xmlns:script:1.0"
 script:name="Module1" script:language="StarBasic">Sub Main
  MsgBox "hi"
End Sub</script:module>"#;

const CONTENT: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
 xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
 office:version="1.3">
<office:body><office:text><text:p>Hello</text:p></office:text></office:body>
</office:document-content>"#;

const STYLES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles
 xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 office:version="1.3"/>"#;

const MANIFEST: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">
<manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.text"/>
<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="Basic/" manifest:media-type="application/binary"/>
<manifest:file-entry manifest:full-path="Basic/Standard/" manifest:media-type="application/binary"/>
<manifest:file-entry manifest:full-path="Basic/Standard/Module1.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="Basic/script-lc.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#;

const SCRIPT_LC: &[u8] = br#"<?xml version="1.0"?><library:libraries/>"#;

fn build_odt_with_basic() -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
    let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"application/vnd.oasis.opendocument.text")
        .unwrap();
    zip.start_file("META-INF/manifest.xml", deflated).unwrap();
    zip.write_all(MANIFEST).unwrap();
    zip.start_file("content.xml", deflated).unwrap();
    zip.write_all(CONTENT).unwrap();
    zip.start_file("styles.xml", deflated).unwrap();
    zip.write_all(STYLES).unwrap();
    zip.start_file("Basic/script-lc.xml", deflated).unwrap();
    zip.write_all(SCRIPT_LC).unwrap();
    zip.start_file("Basic/Standard/Module1.xml", deflated)
        .unwrap();
    zip.write_all(MODULE1).unwrap();
    zip.finish().unwrap();
    buf
}

#[test]
fn import_preserves_basic_library() {
    let doc =
        OdtImport::import(Cursor::new(build_odt_with_basic()), Default::default()).expect("import");
    let macros = doc
        .source
        .as_ref()
        .and_then(|s| s.macros.as_ref())
        .expect("basic library preserved");
    assert_eq!(macros.kind, MacroPayloadKind::OdfBasic);

    let module = macros
        .parts
        .iter()
        .find(|p| p.name == "Basic/Standard/Module1.xml")
        .expect("Module1.xml");
    assert_eq!(module.bytes, MODULE1);

    // Directory entries are preserved (manifest-only, empty bytes).
    assert!(
        macros
            .parts
            .iter()
            .any(|p| p.name == "Basic/" && p.bytes.is_empty())
    );
}

#[test]
fn export_reemits_basic_library_verbatim() {
    let doc =
        OdtImport::import(Cursor::new(build_odt_with_basic()), Default::default()).expect("import");

    let mut out = Cursor::new(Vec::new());
    OdtExport::export(&doc, &mut out, Default::default()).expect("export");

    let reimported =
        OdtImport::import(Cursor::new(out.into_inner()), Default::default()).expect("reimport");
    let macros = reimported
        .source
        .as_ref()
        .and_then(|s| s.macros.as_ref())
        .expect("basic library survives round-trip");

    let module = macros
        .parts
        .iter()
        .find(|p| p.name == "Basic/Standard/Module1.xml")
        .expect("Module1.xml survives");
    assert_eq!(module.bytes, MODULE1, "StarBasic source must be verbatim");

    // Stable payload hash across the round-trip (future trust-store key).
    let original =
        OdtImport::import(Cursor::new(build_odt_with_basic()), Default::default()).unwrap();
    assert_eq!(
        original.source.unwrap().macros.unwrap().payload_hash(),
        macros.payload_hash(),
    );
}

/// End-to-end macro-editor path (spec §3.4, Phase 7.7): import an ODT with a
/// Basic module, edit the module source through the write-back, export, and
/// re-import — the edited source must survive a real ODT save+reopen.
#[test]
fn edited_basic_source_survives_export_and_reimport() {
    use std::collections::BTreeMap;

    let mut doc =
        OdtImport::import(Cursor::new(build_odt_with_basic()), Default::default()).expect("import");

    // Apply an edit to the preserved payload, exactly as the editor's save does.
    let mut payload = doc.source.as_ref().unwrap().macros.clone().unwrap();
    let edits: BTreeMap<String, String> = [(
        "Module1".to_string(),
        "Sub Main\n  Beep\nEnd Sub".to_string(),
    )]
    .into_iter()
    .collect();
    let count = loki_odf::basic_write::apply_basic_edits(&mut payload, &edits).expect("edit");
    assert_eq!(count, 1);
    doc.source.as_mut().unwrap().macros = Some(payload);

    // Save and reopen.
    let mut out = Cursor::new(Vec::new());
    OdtExport::export(&doc, &mut out, Default::default()).expect("export");
    let reimported =
        OdtImport::import(Cursor::new(out.into_inner()), Default::default()).expect("reimport");

    // The reopened document reads back the edited source, not the original.
    let macros = reimported.source.as_ref().unwrap().macros.as_ref().unwrap();
    let modules = loki_odf::basic::extract_basic_modules(macros);
    let module = modules
        .iter()
        .find(|m| m.name == "Module1")
        .expect("Module1 present after reopen");
    assert_eq!(module.source, "Sub Main\n  Beep\nEnd Sub");
    assert!(
        !module.source.contains("MsgBox"),
        "the original source must be gone"
    );
}
