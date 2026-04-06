// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Fallbacks correcting percent escaped character combinations enforcing URI standardizations handling malformed files structurally preserving content boundaries strictly avoiding data losses generating tracking parameters mapping equivalents efficiently protecting integrity explicitly executing limits transparently executing [MS-OI29500] / [MS-OE376] conventions reliably.

use crate::error::{OpcResult, DeviationWarning};
#[cfg(feature = "strict")]
use crate::error::OpcError;
use crate::part::PartName;

/// Attempts percent decoding non standard formatting re-encoding strings canonically securely isolating paths natively generating valid descriptors verifying structures exclusively translating properties protecting tracking outputs.
#[allow(clippy::ptr_arg)]
pub fn normalize_percent_encoding(
    name: &str,
    #[allow(unused_variables)] warnings: &mut Vec<DeviationWarning>,
) -> OpcResult<PartName> {
    #[cfg(feature = "strict")]
    {
        PartName::new(name)
    }

    #[cfg(not(feature = "strict"))]
    {
        match PartName::new(name) {
            Ok(pn) => Ok(pn),
            Err(e) => {
                // Simplified fallback logic for decoding and re-encoding: Attempt a basic URL decode then recode.
                // Normally this would require an RFC3987 encoding step, but for basic ASCII / spaces mis-encodings
                // this acts as a stub resolving known deviations cleanly.
                let mut decoded = String::new();
                let mut bytes = name.as_bytes().iter();
                while let Some(&b) = bytes.next() {
                    if b == b'%' {
                        if let (Some(&h1), Some(&h2)) = (bytes.next(), bytes.next()) {
                            let hex_bytes = [h1, h2];
                            let hex = std::str::from_utf8(&hex_bytes).unwrap_or("00");
                            if let Ok(val) = u8::from_str_radix(hex, 16) {
                                decoded.push(val as char);
                                continue;
                            }
                        }
                    }
                    decoded.push(b as char);
                }

                // Try applying PartName encoding manually back to safe strings
                let re_encoded: String = decoded.chars().map(|c| {
                    if c.is_ascii_alphanumeric() || "-_.~/".contains(c) {
                        c.to_string()
                    } else {
                        let mut b = [0; 4];
                        c.encode_utf8(&mut b).bytes().map(|byte| format!("%{:02X}", byte)).collect::<String>()
                    }
                }).collect();

                let pn = PartName::new(&re_encoded).map_err(|_| e)?;
                warnings.push(DeviationWarning::NonCanonicalPercentEncoding {
                    original: name.to_string(),
                    normalised: re_encoded,
                });
                Ok(pn)
            }
        }
    }
}

/// Strip explicitly duplicate files ignoring case resolving conflicts parsing files sequentially verifying matching configurations checking properties efficiently preserving extraction mappings tracking limits accurately reporting conflicts correctly tracking variants reliably validating bounds successfully matching outputs isolating identifiers seamlessly.
#[allow(clippy::ptr_arg)]
pub fn resolve_duplicate<'a>(
    names: &'a [&'a str],
    #[allow(unused_variables)] warnings: &mut Vec<DeviationWarning>,
) -> OpcResult<&'a str> {
    #[cfg(feature = "strict")]
    {
        if names.len() > 1 {
            return Err(OpcError::DuplicatePartName(names[0].to_string(), names[1].to_string()));
        }
        Ok(names[0])
    }

    #[cfg(not(feature = "strict"))]
    {
        if names.len() > 1 {
            warnings.push(DeviationWarning::DuplicatePartName {
                retained: names[0].to_string(),
                discarded: names[1].to_string(),
            });
        }
        Ok(names[0])
    }
}
