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
    fn options_map_to_ipp_attributes() {
        let options = PrintOptions {
            copies: 3,
            duplex: Duplex::LongEdge,
            media: Some("A4".into()),
            color: ColorMode::Monochrome,
            job_title: None,
        };
        let attrs = options.ipp_attributes().unwrap();
        let rendered: Vec<String> = attrs
            .iter()
            .map(|a| format!("{}={:?}", a.name(), a.value()))
            .collect();
        assert_eq!(attrs.len(), 4, "{rendered:?}");
        assert!(rendered.iter().any(|a| a.starts_with("copies=Integer(3)")));
        assert!(rendered.iter().any(|a| a.contains("two-sided-long-edge")));
        assert!(rendered.iter().any(|a| a.contains("iso_a4_210x297mm")));
        assert!(rendered.iter().any(|a| a.contains("monochrome")));
    }

    #[test]
    fn friendly_media_names_map_to_pwg_keywords() {
        assert_eq!(media_keyword("A4"), "iso_a4_210x297mm");
        assert_eq!(media_keyword("Letter"), "na_letter_8.5x11in");
        // Raw keywords pass through untouched.
        assert_eq!(media_keyword("iso_b5_176x250mm"), "iso_b5_176x250mm");
    }
}
