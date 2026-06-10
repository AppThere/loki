// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF conformance integration tests (part 4).
//!
//! Covers:
//! - `fo:word-spacing` → `CharProps.word_spacing` (gap 14)
//! - `style:letter-kerning` → `CharProps.kerning` (gap 15)
//! - `style:text-scale` → `CharProps.scale` (gap 16)
//! - `style:language-complex` + `style:country-complex` → `CharProps.language_complex` (gap 17)

mod helpers;

use loki_doc_model::style::catalog::StyleId;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};
use std::io::Cursor;

// ── odf14 — fo:word-spacing → word_spacing ───────────────────────────────────

/// `fo:word-spacing="2pt"` must set `CharProps.word_spacing = Some(2pt)`.
/// [ODF 1.3 §20.398]
#[test]
fn odf14_word_spacing_maps_to_char_props() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"SpacedWords\" style:family=\"text\">\
          <style:text-properties fo:word-spacing=\"2pt\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles/>\
      </office:document-styles>";

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p><text:span text:style-name=\"SpacedWords\">Text.</text:span></text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF with fo:word-spacing must import without error");

    let char_style = result
        .document
        .styles
        .character_styles
        .get(&StyleId::new("SpacedWords"))
        .expect("SpacedWords must be in character_styles");

    let ws = char_style
        .char_props
        .word_spacing
        .expect("word_spacing must be Some");
    assert!(
        (ws.value() - 2.0).abs() < 0.01,
        "fo:word-spacing=\"2pt\" must map to ~2.0 pt, got {}",
        ws.value()
    );
}

// ── odf15 — style:letter-kerning → kerning ────────────────────────────────────

/// `style:letter-kerning="true"` must set `CharProps.kerning = Some(true)`.
/// [ODF 1.3 §20.309]
#[test]
fn odf15_letter_kerning_maps_to_char_props() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"KernedText\" style:family=\"text\">\
          <style:text-properties style:letter-kerning=\"true\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles/>\
      </office:document-styles>";

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p><text:span text:style-name=\"KernedText\">Kerned.</text:span></text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF with letter-kerning must import without error");

    let char_style = result
        .document
        .styles
        .character_styles
        .get(&StyleId::new("KernedText"))
        .expect("KernedText must be in character_styles");

    assert_eq!(
        char_style.char_props.kerning,
        Some(true),
        "style:letter-kerning=\"true\" must set CharProps.kerning = Some(true)"
    );
}

// ── odf16 — style:text-scale → scale ─────────────────────────────────────────

/// `style:text-scale="150%"` must set `CharProps.scale = Some(150.0)`.
/// [ODF 1.3 §20.369]
#[test]
fn odf16_text_scale_maps_to_char_props() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"WideText\" style:family=\"text\">\
          <style:text-properties style:text-scale=\"150%\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles/>\
      </office:document-styles>";

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p><text:span text:style-name=\"WideText\">Wide.</text:span></text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF with text-scale must import without error");

    let char_style = result
        .document
        .styles
        .character_styles
        .get(&StyleId::new("WideText"))
        .expect("WideText must be in character_styles");

    let scale = char_style
        .char_props
        .scale
        .expect("scale must be Some after style:text-scale");
    assert!(
        (scale - 150.0).abs() < 0.01,
        "style:text-scale=\"150%\" must map to scale = 150.0, got {scale}"
    );
}

// ── odf17 — language-complex → language_complex ───────────────────────────────

/// `style:language-complex="ar"` + `style:country-complex="SA"` must set
/// `CharProps.language_complex = Some("ar-SA")`. [ODF 1.3 §20.316]
#[test]
fn odf17_language_complex_maps_to_char_props() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"ArabicText\" style:family=\"text\">\
          <style:text-properties \
            style:language-complex=\"ar\" \
            style:country-complex=\"SA\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles/>\
      </office:document-styles>";

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p><text:span text:style-name=\"ArabicText\">Arabic.</text:span></text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF with language-complex must import without error");

    let char_style = result
        .document
        .styles
        .character_styles
        .get(&StyleId::new("ArabicText"))
        .expect("ArabicText must be in character_styles");

    assert_eq!(
        char_style
            .char_props
            .language_complex
            .as_ref()
            .map(|t| t.as_str()),
        Some("ar-SA"),
        "style:language-complex=\"ar\" + country-complex=\"SA\" must set language_complex to ar-SA"
    );
}
