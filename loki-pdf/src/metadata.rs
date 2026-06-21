// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document-level metadata for PDF/X: the XMP packet, the Info dictionary, and
//! the date conversions they share.
//!
//! PDF/X validators read the conformance claim from two places: the Info
//! dictionary key `GTS_PDFXVersion`, and the XMP packet (`pdfxid:` for X-4 and
//! `pdfx:` for X-1a/X-3). Both are written so the file is recognised across
//! the common validators.

use chrono::{DateTime, Datelike, Timelike, Utc};
use pdf_writer::types::TrappingStatus;
use pdf_writer::{Date, Finish, Name, Pdf, Ref, Str, TextStr};

use loki_doc_model::meta::DocumentMeta;

use crate::options::PdfXLevel;

/// The tool string recorded as the producer / creator tool.
pub const PRODUCER: &str = "Loki Text (loki-pdf)";

/// Converts a UTC timestamp to a `pdf_writer::Date`.
#[must_use]
pub fn to_pdf_date(dt: DateTime<Utc>) -> Date {
    Date::new(dt.year() as u16)
        .month(dt.month() as u8)
        .day(dt.day() as u8)
        .hour(dt.hour() as u8)
        .minute(dt.minute() as u8)
        .second(dt.second() as u8)
        .utc_offset_hour(0)
        .utc_offset_minute(0)
}

/// Writes the Info dictionary (registered with the trailer) including the
/// PDF/X conformance marker and the mandatory `Trapped` flag.
pub fn write_info(
    pdf: &mut Pdf,
    info_id: Ref,
    meta: &DocumentMeta,
    title: &str,
    level: PdfXLevel,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
) {
    let mut info = pdf.document_info(info_id);
    info.title(TextStr(title));
    if let Some(author) = meta.creator.as_deref() {
        info.author(TextStr(author));
    }
    if let Some(subject) = meta.subject.as_deref() {
        info.subject(TextStr(subject));
    }
    if let Some(keywords) = meta.keywords.as_deref() {
        info.keywords(TextStr(keywords));
    }
    info.creator(TextStr(PRODUCER));
    info.producer(TextStr(PRODUCER));
    info.creation_date(to_pdf_date(created));
    info.modified_date(to_pdf_date(modified));
    // PDF/X requires the trapping state to be declared explicitly.
    info.trapped(TrappingStatus::NotTrapped);
    // GTS_PDFXVersion is a custom Info key; the writer derefs to the dict.
    info.pair(
        Name(b"GTS_PDFXVersion"),
        Str(level.version_string().as_bytes()),
    );
    info.finish();
}

/// Builds the XMP metadata packet declaring the PDF/X conformance level and
/// the core Dublin Core fields.
#[must_use]
pub fn build_xmp(
    meta: &DocumentMeta,
    title: &str,
    level: PdfXLevel,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
) -> String {
    let created_iso = created.format("%Y-%m-%dT%H:%M:%SZ");
    let modified_iso = modified.format("%Y-%m-%dT%H:%M:%SZ");
    let creator = meta.creator.as_deref().unwrap_or("");
    let description = meta.description.as_deref().unwrap_or("");

    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n\
    xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"\n\
    xmlns:pdfx=\"http://ns.adobe.com/pdfx/1.3/\"\n\
    xmlns:pdfxid=\"http://www.npes.org/pdfx/ns/id/\">\n\
   <dc:format>application/pdf</dc:format>\n\
   <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>\n\
   <dc:creator><rdf:Seq><rdf:li>{creator}</rdf:li></rdf:Seq></dc:creator>\n\
   <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{description}</rdf:li></rdf:Alt></dc:description>\n\
   <xmp:CreatorTool>{producer}</xmp:CreatorTool>\n\
   <xmp:CreateDate>{created}</xmp:CreateDate>\n\
   <xmp:ModifyDate>{modified}</xmp:ModifyDate>\n\
   <pdf:Producer>{producer}</pdf:Producer>\n\
   <pdfx:GTS_PDFXVersion>{version}</pdfx:GTS_PDFXVersion>\n\
   <pdfx:GTS_PDFXConformance>{conformance}</pdfx:GTS_PDFXConformance>\n\
   <pdfxid:GTS_PDFXVersion>{version}</pdfxid:GTS_PDFXVersion>\n\
  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>",
        title = xml_escape(title),
        creator = xml_escape(creator),
        description = xml_escape(description),
        producer = PRODUCER,
        created = created_iso,
        modified = modified_iso,
        version = level.version_string(),
        conformance = level.conformance_string(),
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_contains_conformance_marker() {
        let meta = DocumentMeta::default();
        let now = Utc::now();
        let xmp = build_xmp(&meta, "Doc", PdfXLevel::X3, now, now);
        assert!(xmp.contains("pdfx:GTS_PDFXVersion>PDF/X-3:2003"));
        assert!(xmp.contains("application/pdf"));
    }

    #[test]
    fn escapes_title() {
        let meta = DocumentMeta::default();
        let now = Utc::now();
        let xmp = build_xmp(&meta, "A & B", PdfXLevel::X4, now, now);
        assert!(xmp.contains("A &amp; B"));
        assert!(xmp.contains("pdfxid:GTS_PDFXVersion>PDF/X-4"));
    }
}
