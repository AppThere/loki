// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF conformance integration tests (part 3).
//!
//! Covers `style:writing-mode` → `ParaProps.bidi` (gap 7),
//! `style:font-name-asian` → `CharProps.font_name_east_asian` (gap 10),
//! dropped-frame warning emission (gap 9), and
//! `text:start-value` on list items (gap 8).

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleId;
use loki_odf::error::OdfWarning;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};

// ── odf10 — writing-mode → bidi ───────────────────────────────────────────────

/// `style:writing-mode="rl-tb"` must set `ParaProps.bidi = Some(true)`.
/// [ODF 1.3 §20.394]
#[test]
fn odf10_writing_mode_rl_sets_bidi() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"RtlPara\" style:family=\"paragraph\">\
          <style:paragraph-properties style:writing-mode=\"rl-tb\"/>\
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
        <text:p text:style-name=\"RtlPara\">RTL paragraph.</text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF document with writing-mode must import without error");

    let para_props = result
        .document
        .styles
        .resolve_para(&StyleId::new("RtlPara"))
        .expect("RtlPara must be in the style catalog");

    assert_eq!(
        para_props.bidi,
        Some(true),
        "style:writing-mode=\"rl-tb\" must set ParaProps.bidi = Some(true)"
    );
}

// ── odf11 — font-name-asian → font_name_east_asian ───────────────────────────

/// `style:font-name-asian="MS Mincho"` must set `CharProps.font_name_east_asian`.
/// [ODF 1.3 §20.282]
#[test]
fn odf11_font_name_asian_maps_to_east_asian() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"AsianFont\" style:family=\"text\">\
          <style:text-properties style:font-name-asian=\"MS Mincho\"/>\
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
        <text:p>Text with \
          <text:span text:style-name=\"AsianFont\">East Asian</text:span>.\
        </text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF document with font-name-asian must import without error");

    // ODF `style:family="text"` styles are stored in `character_styles`,
    // not `paragraph_styles`; resolve_char looks in the latter.
    let char_style = result
        .document
        .styles
        .character_styles
        .get(&StyleId::new("AsianFont"))
        .expect("AsianFont must be in character_styles");

    assert_eq!(
        char_style.char_props.font_name_east_asian.as_deref(),
        Some("MS Mincho"),
        "style:font-name-asian must set CharProps.font_name_east_asian"
    );
}

// ── odf12 — dropped frame warning ─────────────────────────────────────────────

/// A `draw:frame` whose content is not `draw:image` or `draw:text-box` must
/// emit `OdfWarning::DroppedFrame` without aborting the import. [ODF 1.3 §10.4]
#[test]
fn odf12_dropped_frame_emits_warning() {
    let styles = helpers::empty_styles_xml("1.2");

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
        xmlns:xlink=\"http://www.w3.org/1999/xlink\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p>Paragraph with an OLE object.\
          <draw:frame draw:name=\"OleFrame\" text:anchor-type=\"as-char\">\
            <draw:object xlink:href=\"./Object1\" xlink:type=\"simple\"/>\
          </draw:frame>\
        </text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("document with unsupported frame must import without error");

    let has_warning = result
        .warnings
        .iter()
        .any(|w| matches!(w, OdfWarning::DroppedFrame { name: Some(n) } if n == "OleFrame"));
    assert!(
        has_warning,
        "OdfWarning::DroppedFrame must be emitted; warnings: {:?}",
        result.warnings
    );
}

// ── odf13 — list item start-value ─────────────────────────────────────────────

/// `text:start-value="3"` on the first `text:list-item` must produce
/// `ListAttributes.start_number = 3`. [ODF 1.3 §5.3.3]
#[test]
fn odf13_list_item_start_value_override() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <text:list-style style:name=\"NumList\">\
          <text:list-level-style-number text:level=\"1\" \
            style:num-format=\"1\" style:num-suffix=\".\"/>\
        </text:list-style>\
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
        <text:list text:style-name=\"NumList\">\
          <text:list-item text:start-value=\"3\">\
            <text:p>Item starting at three.</text:p>\
          </text:list-item>\
          <text:list-item>\
            <text:p>Next item.</text:p>\
          </text:list-item>\
        </text:list>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("list with text:start-value must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let start = all_blocks
        .iter()
        .find_map(|b| {
            if let Block::OrderedList(attrs, _) = b {
                Some(attrs.start_number)
            } else {
                None
            }
        })
        .expect("document must contain an OrderedList block");

    assert_eq!(
        start, 3,
        "text:start-value=\"3\" must override start_number to 3, got {start}"
    );
}
