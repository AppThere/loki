// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Part-name compatibility shims: re-canonicalise non-standard percent encoding
//! and resolve case-insensitive duplicate part names ([MS-OI29500] /
//! [MS-OE376]).

#[cfg(feature = "strict")]
use crate::error::OpcError;
use crate::error::{DeviationWarning, OpcResult};
use crate::part::PartName;

/// Parses `name` into a [`PartName`], repairing non-canonical percent encoding.
///
/// Under `strict` this is a plain [`PartName::new`]. Otherwise, if the raw name
/// is rejected, it is percent-decoded and re-encoded canonically before a
/// second attempt, recording a [`DeviationWarning::NonCanonicalPercentEncoding`]
/// on success. The original parse error is returned if re-encoding still fails.
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
                // Simplified decode/re-encode fallback. A fully conformant
                // implementation would follow RFC 3987, but this handles the
                // common ASCII / space mis-encodings emitted by real writers.
                let mut decoded = String::new();
                let mut bytes = name.as_bytes().iter();
                while let Some(&b) = bytes.next() {
                    if b == b'%'
                        && let (Some(&h1), Some(&h2)) = (bytes.next(), bytes.next())
                    {
                        let hex_bytes = [h1, h2];
                        let hex = std::str::from_utf8(&hex_bytes).unwrap_or("00");
                        if let Ok(val) = u8::from_str_radix(hex, 16) {
                            decoded.push(val as char);
                            continue;
                        }
                    }
                    decoded.push(b as char);
                }

                // Try applying PartName encoding manually back to safe strings
                let re_encoded: String = decoded
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || "-_.~/".contains(c) {
                            c.to_string()
                        } else {
                            let mut b = [0; 4];
                            c.encode_utf8(&mut b)
                                .bytes()
                                .map(|byte| format!("%{:02X}", byte))
                                .collect::<String>()
                        }
                    })
                    .collect();

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

/// Resolves a set of case-insensitively colliding part `names` to the single
/// one to retain (the first).
///
/// Under `strict` a collision is an error ([`OpcError::DuplicatePartName`]).
/// Otherwise the first name is kept and the rest recorded as
/// [`DeviationWarning::DuplicatePartName`].
#[allow(clippy::ptr_arg)]
pub fn resolve_duplicate<'a>(
    names: &'a [&'a str],
    #[allow(unused_variables)] warnings: &mut Vec<DeviationWarning>,
) -> OpcResult<&'a str> {
    #[cfg(feature = "strict")]
    {
        if names.len() > 1 {
            return Err(OpcError::DuplicatePartName(
                names[0].to_string(),
                names[1].to_string(),
            ));
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
