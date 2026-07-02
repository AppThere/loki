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
    use Format::{Docx, Epub, Odg, Odp, Ods, Odt, Pdf, Pptx, Xlsx};
    if matches!(source, Pptx | Odp | Odg) || matches!(target, Pptx | Odp | Odg) {
        return Some(GATED_PRESENTATION);
    }
    match source {
        Epub => return Some("EPUB is export-only; there is no EPUB importer"),
        Pdf => return Some("PDF is export-only; there is no PDF importer"),
        _ => {}
    }
    let text_source = matches!(source, Docx | Odt);
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
        // Text documents convert among themselves and to EPUB/PDF.
        for source in [Format::Docx, Format::Odt] {
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
        // Cross-family and export-only sources are rejected.
        assert!(!is_supported(Format::Docx, Format::Xlsx));
        assert!(!is_supported(Format::Epub, Format::Pdf));
        assert!(!is_supported(Format::Pdf, Format::Docx));
        // The presentation gate (ratified decision 5.1).
        for gated in [Format::Pptx, Format::Odp, Format::Odg] {
            assert!(!is_supported(gated, Format::Pdf));
            assert!(!is_supported(Format::Docx, gated));
        }
        assert_eq!(supported_pairs().len(), 2 * 4 + 2 * 2);
    }
}
