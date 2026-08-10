// SPDX-License-Identifier: Apache-2.0

//! The static capability matrix (ADR-C024).
//!
//! Support is derived from which importers/exporters actually exist, plus
//! the deliberate PPTX/ODP/ODG gate (ratified decision §5.1). A pair absent
//! here is a typed error at the API surface, never a best-effort.

use crate::format::Format;

const GATED_PRESENTATION: &str = "presentation/graphics conversion is gated until the ACID PPTX generator's 29 cases pass \
     (ratified decision 5.1)";

/// Why a pair is unsupported, or `None` when it is supported.
pub(crate) fn unsupported_reason(source: Format, target: Format) -> Option<&'static str> {
    use Format::{Docx, Epub, Fountain, Markdown, Odg, Odp, Ods, Odt, Pdf, Pptx, Xlsx};
    if matches!(source, Pptx | Odp | Odg) || matches!(target, Pptx | Odp | Odg) {
        return Some(GATED_PRESENTATION);
    }
    match source {
        Epub => return Some("EPUB is export-only; there is no EPUB importer"),
        Pdf => return Some("PDF is export-only; there is no PDF importer"),
        _ => {}
    }
    // Markdown/Fountain are the inverse of EPUB/PDF: import-only sources.
    if matches!(target, Markdown | Fountain) {
        return Some("Markdown/Fountain are import-only; there is no exporter");
    }
    let text_source = matches!(source, Docx | Odt | Markdown | Fountain);
    match target {
        Docx | Odt | Epub | Pdf if text_source => None,
        Xlsx | Ods if !text_source => None,
        Xlsx | Ods => Some("spreadsheet targets require a spreadsheet source (XLSX/ODS)"),
        _ => Some(
            "text-document targets require a text-document source; spreadsheets have no \
             layout/PDF path yet",
        ),
    }
}

/// Whether `source → target` is in the capability matrix.
#[must_use]
pub fn is_supported(source: Format, target: Format) -> bool {
    unsupported_reason(source, target).is_none()
}

/// Every supported `(source, target)` pair, for `--list`-style output and
/// conformance-plan iteration.
#[must_use]
pub fn supported_pairs() -> Vec<(Format, Format)> {
    let mut pairs = Vec::new();
    for source in Format::ALL {
        for target in Format::ALL {
            if is_supported(source, target) {
                pairs.push((source, target));
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_matches_the_spec() {
        // Text documents convert among themselves and to EPUB/PDF; the §12
        // import-only sources reach the same text targets.
        for source in [
            Format::Docx,
            Format::Odt,
            Format::Markdown,
            Format::Fountain,
        ] {
            for target in [Format::Docx, Format::Odt, Format::Epub, Format::Pdf] {
                assert!(is_supported(source, target), "{source}->{target}");
            }
        }
        // Spreadsheets convert among themselves only (no PDF path yet).
        for source in [Format::Xlsx, Format::Ods] {
            for target in [Format::Xlsx, Format::Ods] {
                assert!(is_supported(source, target), "{source}->{target}");
            }
            assert!(!is_supported(source, Format::Pdf));
            assert!(!is_supported(source, Format::Docx));
        }
        // Cross-family, export-only sources, and import-only targets are
        // rejected.
        assert!(!is_supported(Format::Docx, Format::Xlsx));
        assert!(!is_supported(Format::Epub, Format::Pdf));
        assert!(!is_supported(Format::Pdf, Format::Docx));
        assert!(!is_supported(Format::Docx, Format::Markdown));
        assert!(!is_supported(Format::Markdown, Format::Fountain));
        assert!(!is_supported(Format::Markdown, Format::Xlsx));
        // The presentation gate (ratified decision 5.1).
        for gated in [Format::Pptx, Format::Odp, Format::Odg] {
            assert!(!is_supported(gated, Format::Pdf));
            assert!(!is_supported(Format::Docx, gated));
        }
        // 4 text sources × 4 text targets + 2 sheet sources × 2 targets.
        assert_eq!(supported_pairs().len(), 4 * 4 + 2 * 2);
    }
}
