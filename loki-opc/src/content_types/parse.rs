// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Extraction bounds structurally matching format types binding identifiers properly parsing internal parameters.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{OpcError, OpcResult};
use crate::part::PartName;
use crate::content_types::ContentTypeMap;

#[cfg(not(feature = "strict"))]
use crate::error::DeviationWarning;

/// Compiles tracking data validating components resolving ISO specification limits iteratively mapping targets internally.
#[allow(clippy::ptr_arg)]
pub fn parse_content_types(
    xml: &[u8],
    #[allow(unused_variables)] warnings: &mut Vec<crate::error::DeviationWarning>,
) -> OpcResult<ContentTypeMap> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut map = ContentTypeMap::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) | Event::Start(e) => {
                let name = e.name();
                if name.as_ref() == b"Default" {
                    let mut ext = None;
                    let mut ct = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Extension" {
                            ext = Some(attr.decode_and_unescape_value(reader.decoder())?.to_string());
                        } else if attr.key.as_ref() == b"ContentType" {
                            ct = Some(attr.decode_and_unescape_value(reader.decoder())?.to_string());
                        }
                    }

                    match (ext, ct) {
                        (Some(x), Some(c)) => {
                            #[allow(unused_mut)]
                            let mut resolved_ct = c.clone();
                            #[cfg(not(feature = "strict"))]
                            if resolved_ct.is_empty() {
                                resolved_ct = crate::compat::content_types::fallback_media_type(&x).to_string();
                                warnings.push(DeviationWarning::MissingMediaType {
                                    part: format!("Default[{}]", x),
                                    fallback: resolved_ct.clone(),
                                });
                            }
                            if resolved_ct.is_empty() {
                                return Err(OpcError::InvalidContentTypes("Empty ContentType strictly forbidden".into()));
                            }
                            map.add_default(&x, &resolved_ct);
                        }
                        _ => return Err(OpcError::InvalidContentTypes("Missing attributes in Default".into())),
                    }
                } else if name.as_ref() == b"Override" {
                    let mut part = None;
                    let mut ct = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"PartName" {
                            let s = attr.decode_and_unescape_value(reader.decoder())?.to_string();
                            part = Some(PartName::new(s).map_err(|_| OpcError::InvalidContentTypes("Invalid Overrides PartName".into()))?);
                        } else if attr.key.as_ref() == b"ContentType" {
                            ct = Some(attr.decode_and_unescape_value(reader.decoder())?.to_string());
                        }
                    }

                    match (part, ct) {
                        (Some(p), Some(c)) => {
                            #[allow(unused_mut)]
                            let mut resolved_ct = c.clone();
                            #[cfg(not(feature = "strict"))]
                            if resolved_ct.is_empty() {
                                let ext = p.extension().unwrap_or("");
                                resolved_ct = crate::compat::content_types::fallback_media_type(ext).to_string();
                                warnings.push(DeviationWarning::MissingMediaType {
                                    part: format!("Override[{}]", p.as_str()),
                                    fallback: resolved_ct.clone(),
                                });
                            }
                            if resolved_ct.is_empty() {
                                return Err(OpcError::InvalidContentTypes("Empty ContentType strictly forbidden".into()));
                            }
                            map.add_override(&p, &resolved_ct);
                        }
                        _ => return Err(OpcError::InvalidContentTypes("Missing attributes in Override".into())),
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(map)
}
