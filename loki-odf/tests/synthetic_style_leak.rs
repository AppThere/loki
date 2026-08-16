// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ODT writer must never emit a synthetic internal style id.
//!
//! The model carries each family's defaults as `__`-prefixed entries
//! (`__Default`, `__DefaultChar`, `__DefaultTable`, and OOXML's
//! `__DocDefault*`). They serialise as `<style:default-style>`, never as a named
//! `<style:style>` — so a `style:parent-style-name="__Default"` in the output
//! names a style the package does not contain.
//!
//! The writer already skipped synthetic style *definitions*. It did not skip
//! *references* to them, which stayed invisible for as long as nothing set such
//! a parent. Making parentless ODF styles inherit from `style:default-style`
//! (ODF 1.3 §16.2, `odt/mapper/styles.rs`) set exactly that parent, and this
//! whole crate's suite still passed — the leak appears in no correctness diff,
//! only in the bytes.

use std::io::{Cursor, Read};

use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_odf::odt::export::{OdtExport, OdtExportOptions};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};

/// Imports `bytes`, exports the result, and returns `(content.xml, styles.xml)`.
fn round_trip(bytes: Vec<u8>) -> (String, String) {
    let doc = OdtImport::import(Cursor::new(bytes), OdtImportOptions::default())
        .expect("fixture imports");
    let mut out = Cursor::new(Vec::new());
    OdtExport::export(&doc, &mut out, OdtExportOptions::default()).expect("exports");
    let mut zip = zip::ZipArchive::new(Cursor::new(out.into_inner())).expect("valid zip");
    let mut parts = Vec::new();
    for name in ["content.xml", "styles.xml"] {
        let mut s = String::new();
        zip.by_name(name)
            .expect("part present")
            .read_to_string(&mut s)
            .expect("part decodes");
        parts.push(s);
    }
    (parts[0].clone(), parts[1].clone())
}

/// A fixture whose only font declaration is on `style:default-style`, with a
/// parentless style in each of the three mapped families — the shape that makes
/// the mapper set a synthetic parent.
fn default_style_fixture() -> Vec<u8> {
    const STYLES: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<office:document-styles "#,
        r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
        r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
        r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
        r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
        r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
        r#"office:version="1.2"><office:styles>"#,
        r#"<style:default-style style:family="paragraph">"#,
        r#"<style:text-properties fo:font-family="Tinos" fo:font-size="12pt"/>"#,
        r#"</style:default-style>"#,
        r#"<style:default-style style:family="text">"#,
        r#"<style:text-properties fo:font-family="Tinos"/>"#,
        r#"</style:default-style>"#,
        r#"<style:default-style style:family="table">"#,
        r#"<style:table-properties style:rel-width="50%"/>"#,
        r#"</style:default-style>"#,
        r#"<style:style style:name="Body" style:family="paragraph">"#,
        r#"<style:text-properties fo:font-size="14pt"/>"#,
        r#"</style:style>"#,
        r#"<style:style style:name="Emph" style:family="text">"#,
        r#"<style:text-properties fo:font-style="italic"/>"#,
        r#"</style:style>"#,
        r#"<style:style style:name="Tbl" style:family="table">"#,
        r#"<style:table-properties table:align="center"/>"#,
        r#"</style:style>"#,
        r#"</office:styles></office:document-styles>"#,
    );
    const CONTENT: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<office:document-content "#,
        r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
        r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
        r#"office:version="1.2"><office:body><office:text>"#,
        r#"<text:p text:style-name="Body">Hello</text:p>"#,
        r#"</office:text></office:body></office:document-content>"#,
    );
    const MANIFEST: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<manifest:manifest "#,
        r#"xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" "#,
        r#"manifest:version="1.2">"#,
        r#"<manifest:file-entry manifest:full-path="/" "#,
        r#"manifest:media-type="application/vnd.oasis.opendocument.text"/>"#,
        r#"<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>"#,
        r#"<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>"#,
        r#"</manifest:manifest>"#,
    );

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let stored = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        use std::io::Write;
        zip.start_file("mimetype", stored).expect("mimetype");
        zip.write_all(b"application/vnd.oasis.opendocument.text")
            .expect("mimetype body");
        for (name, body) in [
            ("META-INF/manifest.xml", MANIFEST),
            ("content.xml", CONTENT),
            ("styles.xml", STYLES),
        ] {
            zip.start_file(name, deflated).expect("part");
            zip.write_all(body.as_bytes()).expect("part body");
        }
        zip.finish().expect("zip finishes");
    }
    buf.into_inner()
}

#[test]
fn export_never_emits_a_synthetic_style_id() {
    let (content, styles) = round_trip(default_style_fixture());
    for (part, xml) in [("content.xml", &content), ("styles.xml", &styles)] {
        assert!(
            !xml.contains("__Default"),
            "{part} references the synthetic default style:\n{xml}"
        );
        assert!(
            !xml.contains("style:parent-style-name=\"__"),
            "{part} emits a synthetic parent reference:\n{xml}"
        );
    }
}

/// The guard: the fixture really does exercise the path. If the mapper stopped
/// setting a synthetic parent, the assertion above would pass vacuously and
/// stop being evidence of anything.
#[test]
fn the_fixture_actually_sets_a_synthetic_parent() {
    use loki_doc_model::style::catalog::StyleId;
    let doc = OdtImport::import(
        Cursor::new(default_style_fixture()),
        OdtImportOptions::default(),
    )
    .expect("fixture imports");
    let s = &doc.styles;

    // All three families, or the leak assertion passes vacuously for the two
    // the fixture forgot to exercise — which is exactly how the character and
    // table writers kept an unguarded parent long after the paragraph one was
    // fixed.
    assert_eq!(
        s.paragraph_styles
            .get(&StyleId::new("Body"))
            .expect("Body mapped")
            .parent
            .as_ref()
            .map(|p| p.as_str()),
        Some("__Default"),
        "parentless paragraph style must inherit the family default (ODF 1.3 §16.2)"
    );
    assert_eq!(
        s.character_styles
            .get(&StyleId::new("Emph"))
            .expect("Emph mapped")
            .parent
            .as_ref()
            .map(|p| p.as_str()),
        Some("__DefaultChar"),
        "parentless text style must inherit the family default (ODF 1.3 §16.2)"
    );
    assert_eq!(
        s.table_styles
            .get(&StyleId::new("Tbl"))
            .expect("Tbl mapped")
            .parent
            .as_ref()
            .map(|p| p.as_str()),
        Some("__DefaultTable"),
        "parentless table style must inherit the family default (ODF 1.3 §16.2)"
    );
}

/// A real committed fixture, through the same check — the synthetic parent is
/// reached by ordinary documents, not only by a hand-built one.
#[test]
fn committed_fixture_round_trips_without_synthetic_ids() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../appthere-conformance/fixtures/odt/para-carlito.odt"
    );
    let Ok(bytes) = std::fs::read(path) else {
        // Generated by `loki-odf`'s gen_conformance_fixtures example; skip
        // rather than fail when the tree has not been generated yet.
        eprintln!("skipping: {path} not present");
        return;
    };
    let (content, styles) = round_trip(bytes);
    assert!(!content.contains("__"), "content.xml leaked a synthetic id");
    assert!(!styles.contains("__Default"), "styles.xml leaked __Default");
}
