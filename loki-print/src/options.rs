// SPDX-License-Identifier: Apache-2.0

//! Print job options and their mapping onto IPP job attributes.

use ipp::attribute::IppAttribute;
use ipp::value::IppValue;

use crate::error::PrintError;

/// Duplex mode (IPP `sides`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Duplex {
    /// Single-sided.
    #[default]
    Simplex,
    /// Double-sided, flip on the long edge (portrait duplex).
    LongEdge,
    /// Double-sided, flip on the short edge (landscape duplex).
    ShortEdge,
}

impl Duplex {
    const fn keyword(self) -> &'static str {
        match self {
            Duplex::Simplex => "one-sided",
            Duplex::LongEdge => "two-sided-long-edge",
            Duplex::ShortEdge => "two-sided-short-edge",
        }
    }
}

/// Colour mode (IPP `print-color-mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Printer default.
    #[default]
    Auto,
    /// Force colour.
    Color,
    /// Force monochrome.
    Monochrome,
}

impl ColorMode {
    const fn keyword(self) -> Option<&'static str> {
        match self {
            ColorMode::Auto => None,
            ColorMode::Color => Some("color"),
            ColorMode::Monochrome => Some("monochrome"),
        }
    }
}

/// Options for one print job (headless spec §3).
#[derive(Debug, Clone, Default)]
pub struct PrintOptions {
    /// Number of copies (IPP `copies`; `0`/`1` are both "one copy").
    pub copies: u32,
    /// Duplex mode.
    pub duplex: Duplex,
    /// Media size — a friendly name (`A4`, `letter`, …) or a raw IPP media
    /// keyword (`iso_a4_210x297mm`).
    pub media: Option<String>,
    /// Colour mode.
    pub color: ColorMode,
    /// Job title shown in the printer queue.
    pub job_title: Option<String>,
    /// Pages to print, `"1-3,5"` style (1-based, inclusive; a bare `N` is the
    /// single page `N`, `N-M` a closed range). `None` prints everything.
    /// IPP `page-ranges` (RFC 8011 §5.2.7). Parsed and validated by
    /// [`parse_page_ranges`] at attribute-build time, so a malformed string is
    /// a typed [`PrintError::InvalidOption`] before any bytes reach a printer.
    pub page_ranges: Option<String>,
}

/// Parses a `"1-3,5"` page-range string into 1-based inclusive `(from, to)`
/// pairs. Rejects empty segments, zero pages (IPP ranges are 1-based),
/// inverted ranges, and non-numeric input — every rejection names the
/// offending segment.
pub fn parse_page_ranges(spec: &str) -> Result<Vec<(i32, i32)>, PrintError> {
    let invalid = |seg: &str| PrintError::InvalidOption(format!("page-ranges segment {seg:?}"));
    let mut ranges = Vec::new();
    for segment in spec.split(',') {
        let segment = segment.trim();
        let (from, to) = match segment.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (segment, segment),
        };
        let from: i32 = from.parse().map_err(|_| invalid(segment))?;
        let to: i32 = to.parse().map_err(|_| invalid(segment))?;
        if from < 1 || to < from {
            return Err(invalid(segment));
        }
        ranges.push((from, to));
    }
    if ranges.is_empty() {
        return Err(PrintError::InvalidOption("page-ranges is empty".into()));
    }
    Ok(ranges)
}

/// Maps friendly media names to IPP self-describing media keywords
/// (PWG 5101.1); unrecognised values pass through as raw keywords.
#[must_use]
pub(crate) fn media_keyword(media: &str) -> &str {
    match media.to_ascii_lowercase().as_str() {
        "a3" => "iso_a3_297x420mm",
        "a4" => "iso_a4_210x297mm",
        "a5" => "iso_a5_148x210mm",
        "letter" => "na_letter_8.5x11in",
        "legal" => "na_legal_8.5x14in",
        "tabloid" | "ledger" => "na_ledger_11x17in",
        _ => media,
    }
}

impl PrintOptions {
    /// Encodes these options as IPP job attributes.
    pub(crate) fn ipp_attributes(&self) -> Result<Vec<IppAttribute>, PrintError> {
        fn keyword(name: &str, value: &str) -> Result<IppAttribute, PrintError> {
            let value = IppValue::Keyword(
                value
                    .try_into()
                    .map_err(|_| PrintError::InvalidOption(format!("{name}={value}")))?,
            );
            IppAttribute::with_name(name, value)
                .map_err(|_| PrintError::InvalidOption(name.to_owned()))
        }

        let mut attributes = Vec::new();
        if self.copies > 1 {
            let copies = i32::try_from(self.copies)
                .map_err(|_| PrintError::InvalidOption(format!("copies={}", self.copies)))?;
            attributes.push(
                IppAttribute::with_name(IppAttribute::COPIES, IppValue::Integer(copies))
                    .map_err(|_| PrintError::InvalidOption("copies".to_owned()))?,
            );
        }
        if self.duplex != Duplex::Simplex {
            attributes.push(keyword(IppAttribute::SIDES, self.duplex.keyword())?);
        }
        if let Some(media) = &self.media {
            // The ipp crate exposes MEDIA_COL but not plain `media`.
            attributes.push(keyword("media", media_keyword(media))?);
        }
        if let Some(color) = self.color.keyword() {
            attributes.push(keyword("print-color-mode", color)?);
        }
        if let Some(spec) = &self.page_ranges {
            let ranges: Vec<IppValue> = parse_page_ranges(spec)?
                .into_iter()
                .map(|(min, max)| IppValue::RangeOfInteger { min, max })
                .collect();
            // A single range is sent bare; several as a 1setOf.
            let value = match <[IppValue; 1]>::try_from(ranges) {
                Ok([single]) => single,
                Err(many) => IppValue::Array(many),
            };
            attributes.push(
                IppAttribute::with_name("page-ranges", value)
                    .map_err(|_| PrintError::InvalidOption("page-ranges".to_owned()))?,
            );
        }
        Ok(attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_add_no_attributes() {
        assert!(PrintOptions::default().ipp_attributes().unwrap().is_empty());
    }

    #[test]
    fn zero_and_one_copies_emit_no_copies_attribute() {
        // IPP `copies` is integer(1:MAX), so 0 must never go on the wire. The
        // encoder's `> 1` guard is what keeps a 0 from a caller (a print
        // dialog's "0" field) from becoming a zero-copy job; without a test
        // on the guard's *false* side, widening it to `>= 1` — or to `> 0`,
        // which would send `copies=0` — fails nothing.
        let names = |copies: u32| -> Vec<String> {
            PrintOptions {
                copies,
                ..PrintOptions::default()
            }
            .ipp_attributes()
            .unwrap()
            .iter()
            .map(|a| a.name().to_string())
            .collect()
        };
        for copies in [0, 1] {
            assert!(
                !names(copies).iter().any(|n| n == IppAttribute::COPIES),
                "copies={copies} must not emit a copies attribute"
            );
        }
        // The polarity: 2 does emit it, so the assertion above is about the
        // boundary rather than about copies never being sent.
        assert!(names(2).iter().any(|n| n == IppAttribute::COPIES));
    }

    #[test]
    fn options_map_to_ipp_attributes() {
        let options = PrintOptions {
            copies: 3,
            duplex: Duplex::LongEdge,
            media: Some("A4".into()),
            color: ColorMode::Monochrome,
            job_title: None,
            page_ranges: Some("1-3,5".into()),
        };
        let attrs = options.ipp_attributes().unwrap();
        let rendered: Vec<String> = attrs
            .iter()
            .map(|a| format!("{}={:?}", a.name(), a.value()))
            .collect();
        assert_eq!(attrs.len(), 5, "{rendered:?}");
        assert!(rendered.iter().any(|a| a.starts_with("page-ranges=")));
        assert!(rendered.iter().any(|a| a.starts_with("copies=Integer(3)")));
        assert!(rendered.iter().any(|a| a.contains("two-sided-long-edge")));
        assert!(rendered.iter().any(|a| a.contains("iso_a4_210x297mm")));
        assert!(rendered.iter().any(|a| a.contains("monochrome")));
    }

    #[test]
    fn page_ranges_parse_and_reject() {
        assert_eq!(parse_page_ranges("1-3,5").unwrap(), vec![(1, 3), (5, 5)]);
        assert_eq!(parse_page_ranges(" 2 - 4 ").unwrap(), vec![(2, 4)]);
        // The inversions: zero page, inverted range, junk, empty.
        for bad in ["0", "3-2", "1-", "a-b", "", "1,,2"] {
            assert!(
                parse_page_ranges(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
        // A malformed spec surfaces at attribute build, not at the printer.
        let options = PrintOptions {
            page_ranges: Some("3-2".into()),
            ..Default::default()
        };
        assert!(options.ipp_attributes().is_err());
    }

    #[test]
    fn friendly_media_names_map_to_pwg_keywords() {
        assert_eq!(media_keyword("A4"), "iso_a4_210x297mm");
        assert_eq!(media_keyword("Letter"), "na_letter_8.5x11in");
        // Raw keywords pass through untouched.
        assert_eq!(media_keyword("iso_b5_176x250mm"), "iso_b5_176x250mm");
    }
}
