// SPDX-License-Identifier: Apache-2.0

//! End-to-end conversions through real format bytes: build a document /
//! workbook, export it, run it through the matrix, and validate the output
//! parses as the target format (ADR-C024).

use std::io::Cursor;

use loki_convert::{ConvertError, ConvertOptions, Format, PdfProfile, convert};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_ooxml::{DocxExport, DocxImport};
use loki_sheet_model::workbook::{Workbook, Worksheet};

fn sample_docx() -> Vec<u8> {
    let mut doc = Document::new();
    doc.meta.title = Some("Conversion fixture".into());
    if let Some(section) = doc.sections.first_mut() {
        section.blocks.push(Block::Heading(
            1,
            Default::default(),
            vec![Inline::Str("Quarterly report".into())],
        ));
        section.blocks.push(Block::Para(vec![Inline::Str(
            "Printed and converted without a GPU.".into(),
        )]));
    }
    let mut cursor = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut cursor, ()).expect("fixture docx");
    cursor.into_inner()
}

fn sample_ods() -> Vec<u8> {
    let mut workbook = Workbook::new();
    workbook.sheets.push(Worksheet::new("Sheet1"));
    let mut cursor = Cursor::new(Vec::new());
    loki_odf::OdsExport::export(&workbook, &mut cursor).expect("fixture ods");
    cursor.into_inner()
}

#[test]
fn docx_to_odt_to_docx_preserves_text() {
    let docx = sample_docx();
    let odt = convert(Format::Docx, &docx, Format::Odt, &ConvertOptions::default()).unwrap();
    assert_eq!(&odt.bytes[..2], b"PK");

    let back = convert(
        Format::Odt,
        &odt.bytes,
        Format::Docx,
        &ConvertOptions::default(),
    )
    .unwrap();
    let doc = DocxImport::import(Cursor::new(&back.bytes), Default::default()).unwrap();
    let text = format!("{:?}", doc.sections);
    assert!(
        text.contains("Quarterly report"),
        "heading lost in round trip"
    );
    assert!(
        text.contains("without a GPU"),
        "paragraph lost in round trip"
    );
}

#[test]
fn docx_to_pdf_emits_requested_profile() {
    let docx = sample_docx();
    let options = ConvertOptions {
        pdf_profile: PdfProfile::PdfX4,
        title: Some("Print run".into()),
    };
    let pdf = convert(Format::Docx, &docx, Format::Pdf, &options).unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF-1."), "missing PDF header");
    let text = String::from_utf8_lossy(&pdf.bytes);
    assert!(text.contains("PDF/X-4"), "PDF/X-4 marker missing");
}

#[test]
fn docx_to_epub_produces_epub_container() {
    let docx = sample_docx();
    let epub = convert(
        Format::Docx,
        &docx,
        Format::Epub,
        &ConvertOptions::default(),
    )
    .unwrap();
    // OCF: ZIP whose first entry is the stored `mimetype` file.
    assert_eq!(&epub.bytes[..2], b"PK");
    assert!(
        epub.bytes.windows(20).any(|w| w == b"application/epub+zip"),
        "EPUB mimetype missing"
    );
}

#[test]
fn ods_and_xlsx_round_trip() {
    let ods = sample_ods();
    let xlsx = convert(Format::Ods, &ods, Format::Xlsx, &ConvertOptions::default()).unwrap();
    assert_eq!(&xlsx.bytes[..2], b"PK");
    let back = convert(
        Format::Xlsx,
        &xlsx.bytes,
        Format::Ods,
        &ConvertOptions::default(),
    )
    .unwrap();
    assert_eq!(&back.bytes[..2], b"PK");
}

#[test]
fn unsupported_pairs_are_typed_and_parse_nothing() {
    // Cross-family: even valid bytes are rejected before parsing.
    let err = convert(
        Format::Docx,
        b"garbage",
        Format::Xlsx,
        &ConvertOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(err, ConvertError::ConversionUnsupported { .. }));

    // The presentation gate (ratified decision 5.1).
    let err = convert(
        Format::Pptx,
        b"garbage",
        Format::Pdf,
        &ConvertOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(err, ConvertError::ConversionUnsupported { .. }));
}

#[test]
fn pdf_a2b_is_honestly_unsupported() {
    let docx = sample_docx();
    let options = ConvertOptions {
        pdf_profile: PdfProfile::PdfA2b,
        title: None,
    };
    let err = convert(Format::Docx, &docx, Format::Pdf, &options).unwrap_err();
    assert!(matches!(err, ConvertError::ProfileUnsupported("pdf-a2b")));
}

#[test]
fn profile_on_non_pdf_target_is_rejected() {
    let docx = sample_docx();
    let options = ConvertOptions {
        pdf_profile: PdfProfile::PdfX4,
        title: None,
    };
    let err = convert(Format::Docx, &docx, Format::Odt, &options).unwrap_err();
    assert!(matches!(err, ConvertError::ProfileWithoutPdfTarget));
}

/// A macro-enabled `.docm` converted to a macro-free target warns that the
/// VBA payload was dropped, rather than dropping it silently (spec §3.5).
#[test]
fn dropping_macros_on_conversion_warns() {
    use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
    use loki_doc_model::io::source::DocumentSource;
    use loki_ooxml::DocxMacroEnabledExport;

    let mut doc = Document::new();
    if let Some(section) = doc.sections.first_mut() {
        section
            .blocks
            .push(Block::Para(vec![Inline::Str("body".into())]));
    }
    let mut source = DocumentSource::new("ooxml");
    source.macros = Some(MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            Some("application/vnd.ms-office.vbaProject".into()),
            b"\xd0\xcf\x11\xe0FAKE".to_vec(),
        )],
    ));
    doc.source = Some(source);

    let mut docm = Cursor::new(Vec::new());
    DocxMacroEnabledExport::export(&doc, &mut docm, ()).expect("docm");

    let out = convert(
        Format::Docx,
        &docm.into_inner(),
        Format::Odt,
        &ConvertOptions::default(),
    )
    .expect("convert");
    assert!(
        out.warnings.iter().any(|w| w.contains("macros dropped")),
        "expected a macros-dropped warning, got: {:?}",
        out.warnings
    );
}
