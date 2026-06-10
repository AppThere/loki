// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF conformance integration tests (part 5).
//!
//! Covers:
//! - `style:language-asian` + `style:country-asian` → `CharProps.language_east_asian` (gap 18)
//! - `style:leader-style="dotted"` → `TabStop.leader = TabLeader::Dot` (gap 19)
//! - `text:alphabetical-index` body element emits `OdfWarning::UnrecognisedElement` (gap 20)

mod helpers;

use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::tab_stop::TabLeader;
use loki_odf::error::OdfWarning;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};
use std::io::Cursor;

// ── odf18 — language-asian → language_east_asian ─────────────────────────────

/// `style:language-asian="ja"` + `style:country-asian="JP"` must set
/// `CharProps.language_east_asian = Some("ja-JP")`. [ODF 1.3 §20.314]
#[test]
fn odf18_language_asian_maps_to_language_east_asian() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"JapaneseText\" style:family=\"text\">\
          <style:text-properties \
            style:language-asian=\"ja\" \
            style:country-asian=\"JP\"/>\
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
        <text:p><text:span text:style-name=\"JapaneseText\">Japanese.</text:span></text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF with language-asian must import without error");

    let char_style = result
        .document
        .styles
        .character_styles
        .get(&StyleId::new("JapaneseText"))
        .expect("JapaneseText must be in character_styles");

    assert_eq!(
        char_style
            .char_props
            .language_east_asian
            .as_ref()
            .map(|t| t.as_str()),
        Some("ja-JP"),
        "style:language-asian=\"ja\" + country-asian=\"JP\" must set language_east_asian to ja-JP"
    );
}

// ── odf19 — style:leader-style → TabStop.leader ──────────────────────────────

/// `style:leader-style="dotted"` on a tab stop must set
/// `TabStop.leader = TabLeader::Dot`. [ODF 1.3 §20.319]
#[test]
fn odf19_tab_leader_dotted_maps_to_tab_leader_dot() {
    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"TabLeaderPara\" style:family=\"paragraph\">\
          <style:paragraph-properties>\
            <style:tab-stops>\
              <style:tab-stop \
                style:position=\"5cm\" \
                style:type=\"right\" \
                style:leader-style=\"dotted\"/>\
            </style:tab-stops>\
          </style:paragraph-properties>\
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
        <text:p text:style-name=\"TabLeaderPara\">TOC entry.</text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF with leader-style tab stop must import without error");

    let para_props = result
        .document
        .styles
        .resolve_para(&StyleId::new("TabLeaderPara"))
        .expect("TabLeaderPara must be in the style catalog");

    let stops = para_props
        .tab_stops
        .as_ref()
        .expect("tab_stops must be Some");

    let leader = stops
        .first()
        .map(|ts| ts.leader)
        .expect("at least one tab stop must be present");

    assert_eq!(
        leader,
        TabLeader::Dot,
        "style:leader-style=\"dotted\" must map to TabLeader::Dot"
    );
}

// ── odf20 — index block emits UnrecognisedElement warning ────────────────────

/// A `text:alphabetical-index` body element must not abort the import and must
/// emit an `OdfWarning::UnrecognisedElement` with the element name. [ODF 1.3 §8]
#[test]
fn odf20_alphabetical_index_emits_warning() {
    let styles = helpers::empty_styles_xml("1.2");

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p>Before index.</text:p>\
        <text:alphabetical-index text:name=\"Index1\">\
          <text:alphabetical-index-body>\
            <text:p>Index entry.</text:p>\
          </text:alphabetical-index-body>\
        </text:alphabetical-index>\
        <text:p>After index.</text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, &styles, None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("document with alphabetical-index must import without error");

    let has_warning = result.warnings.iter().any(|w| {
        matches!(w, OdfWarning::UnrecognisedElement { element, .. } if element == "alphabetical-index")
    });
    assert!(
        has_warning,
        "OdfWarning::UnrecognisedElement for alphabetical-index must be emitted; warnings: {:?}",
        result.warnings
    );
}
