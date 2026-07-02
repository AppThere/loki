// SPDX-License-Identifier: Apache-2.0

//! The conversion pipeline: import source → in-memory model → export target.

use std::io::Cursor;

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_epub::{EpubExport, EpubOptions};
use loki_odf::odt::import::OdtImporter;
use loki_odf::{OdsExport, OdsImport, OdsImportOptions, OdtExport, OdtImportOptions};
use loki_ooxml::docx::import::DocxImporter;
use loki_ooxml::{DocxExport, DocxImportOptions, XlsxExport, XlsxImport, XlsxImportOptions};
use loki_pdf::{PdfXLevel, PdfXOptions};
use loki_sheet_model::workbook::Workbook;

use crate::error::ConvertError;
use crate::format::{Format, PdfProfile};
use crate::matrix::unsupported_reason;

/// Options applied across the whole conversion.
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// PDF conformance profile; only meaningful when the target is PDF.
    pub pdf_profile: PdfProfile,
    /// Overrides the document title in formats that carry one.
    pub title: Option<String>,
}

/// A successful conversion.
#[derive(Debug)]
pub struct ConvertOutput {
    /// The converted file bytes.
    pub bytes: Vec<u8>,
    /// Non-fatal import warnings (unrecognised elements, unresolved
    /// relationships, …) — surfaced so batch jobs can log quality issues.
    pub warnings: Vec<String>,
}

/// Converts `input` (bytes of a `source`-format file) into `target` format.
///
/// Unsupported pairs return [`ConvertError::ConversionUnsupported`] before
/// any parsing happens (ADR-C024).
pub fn convert(
    source: Format,
    input: &[u8],
    target: Format,
    options: &ConvertOptions,
) -> Result<ConvertOutput, ConvertError> {
    if let Some(reason) = unsupported_reason(source, target) {
        return Err(ConvertError::ConversionUnsupported {
            source_format: source,
            target_format: target,
            reason,
        });
    }
    if options.pdf_profile != PdfProfile::Default && target != Format::Pdf {
        return Err(ConvertError::ProfileWithoutPdfTarget);
    }
    match source {
        Format::Docx | Format::Odt => {
            let text_source = if source == Format::Docx {
                TextSource::Docx
            } else {
                TextSource::Odt
            };
            let (mut doc, warnings) = import_text(text_source, input)?;
            if let Some(title) = &options.title {
                doc.meta.title = Some(title.clone());
            }
            let bytes = export_text(&doc, source, target, options)?;
            Ok(ConvertOutput { bytes, warnings })
        }
        Format::Xlsx | Format::Ods => {
            let workbook = import_sheet(source, input)?;
            let bytes = export_sheet(&workbook, target)?;
            Ok(ConvertOutput {
                bytes,
                warnings: Vec::new(),
            })
        }
        // unsupported_reason() already rejected every other source; this arm
        // exists only to keep the match exhaustive without panicking.
        _ => Err(ConvertError::ConversionUnsupported {
            source_format: source,
            target_format: target,
            reason: "source format has no import path",
        }),
    }
}

/// Text-document sources, narrowed after the matrix check.
enum TextSource {
    Docx,
    Odt,
}

fn import_text(source: TextSource, input: &[u8]) -> Result<(Document, Vec<String>), ConvertError> {
    let cursor = Cursor::new(input);
    match source {
        TextSource::Docx => {
            let result = DocxImporter::new(DocxImportOptions::default()).run(cursor)?;
            let warnings = result.warnings.iter().map(|w| format!("{w:?}")).collect();
            Ok((result.document, warnings))
        }
        TextSource::Odt => {
            let result = OdtImporter::new(OdtImportOptions::default()).run(cursor)?;
            let warnings = result.warnings.iter().map(|w| format!("{w:?}")).collect();
            Ok((result.document, warnings))
        }
    }
}

fn export_text(
    doc: &Document,
    source: Format,
    target: Format,
    options: &ConvertOptions,
) -> Result<Vec<u8>, ConvertError> {
    let mut cursor = Cursor::new(Vec::new());
    match target {
        Format::Docx => DocxExport::export(doc, &mut cursor, ())?,
        Format::Odt => OdtExport::export(doc, &mut cursor, Default::default())?,
        Format::Epub => EpubExport::export(doc, &mut cursor, EpubOptions::default())?,
        Format::Pdf => {
            let level = match options.pdf_profile {
                PdfProfile::Default | PdfProfile::PdfX1a => PdfXLevel::X1a,
                PdfProfile::PdfX3 => PdfXLevel::X3,
                PdfProfile::PdfX4 => PdfXLevel::X4,
                // TODO(headless-c022): PDF/A-2b needs the krilla engine
                // migration; the current pdf-writer engine emits PDF/X only.
                PdfProfile::PdfA2b => return Err(ConvertError::ProfileUnsupported("pdf-a2b")),
            };
            let pdf_options = PdfXOptions {
                level,
                title: options.title.clone(),
                ..Default::default()
            };
            let mut out = Vec::new();
            loki_pdf::export_document(doc, &pdf_options, &mut out)?;
            return Ok(out);
        }
        // The matrix admits only the targets above for text sources.
        other => {
            return Err(ConvertError::ConversionUnsupported {
                source_format: source,
                target_format: other,
                reason: "target format has no text-document export path",
            });
        }
    }
    Ok(cursor.into_inner())
}

fn import_sheet(source: Format, input: &[u8]) -> Result<Workbook, ConvertError> {
    let cursor = Cursor::new(input);
    match source {
        Format::Ods => Ok(OdsImport::import(cursor, OdsImportOptions::default())?),
        // The caller only passes Xlsx or Ods; default to the XLSX importer.
        _ => Ok(XlsxImport::import(cursor, XlsxImportOptions::default())?),
    }
}

fn export_sheet(workbook: &Workbook, target: Format) -> Result<Vec<u8>, ConvertError> {
    let mut cursor = Cursor::new(Vec::new());
    match target {
        Format::Ods => OdsExport::export(workbook, &mut cursor)?,
        // The matrix admits only Xlsx | Ods here.
        _ => XlsxExport::export(workbook, &mut cursor)?,
    }
    Ok(cursor.into_inner())
}
