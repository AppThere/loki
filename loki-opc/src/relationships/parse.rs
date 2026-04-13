// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! `quick-xml` event mode parsing implementing ISO metadata structure extraction targeting relationship types specifically.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{OpcError, OpcResult};
use crate::relationships::{Relationship, TargetMode};

#[cfg(not(feature = "strict"))]
use crate::error::DeviationWarning;

/// Extract and instantiate mappings from relationships streams.
#[allow(clippy::ptr_arg)]
pub fn parse_relationships_part(
    xml: &[u8],
    part_name: &str,
    #[allow(unused_variables)] warnings: &mut Vec<crate::error::DeviationWarning>,
) -> OpcResult<Vec<Relationship>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut relationships = Vec::new();
    #[cfg(not(feature = "strict"))]
    let mut seen_ids = std::collections::HashSet::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) | Event::Start(e) => {
                if e.name().as_ref() == b"Relationship" {
                    let mut id = None;
                    let mut rel_type = None;
                    let mut target = None;
                    let mut target_mode = TargetMode::Internal;

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" => {
                                id = Some(
                                    attr.decode_and_unescape_value(reader.decoder())?.to_string(),
                                );
                            }
                            b"Type" => {
                                rel_type = Some(
                                    attr.decode_and_unescape_value(reader.decoder())?.to_string(),
                                );
                            }
                            b"Target" => {
                                target = Some(
                                    attr.decode_and_unescape_value(reader.decoder())?.to_string(),
                                );
                            }
                            b"TargetMode" => {
                                let val = attr.decode_and_unescape_value(reader.decoder())?;
                                if val == "External" {
                                    target_mode = TargetMode::External;
                                }
                            }
                            _ => {}
                        }
                    }

                    let id = id.ok_or_else(|| OpcError::InvalidRelationships {
                        part: part_name.to_string(),
                        reason: "Relationship element missing Id attribute".to_string(),
                    })?;
                    let rel_type = rel_type.ok_or_else(|| OpcError::InvalidRelationships {
                        part: part_name.to_string(),
                        reason: "Relationship element missing Type attribute".to_string(),
                    })?;
                    let target = target.ok_or_else(|| OpcError::InvalidRelationships {
                        part: part_name.to_string(),
                        reason: "Relationship element missing Target attribute".to_string(),
                    })?;

                    #[cfg(not(feature = "strict"))]
                    {
                        if !seen_ids.insert(id.clone()) {
                            warnings.push(DeviationWarning::DuplicateRelationshipId {
                                id: id.clone(),
                                part: part_name.to_string(),
                            });
                            continue; // Discard subsequent duplicate IDs explicitly parsing structures natively.
                        }
                    }

                    relationships.push(Relationship {
                        id,
                        rel_type,
                        target,
                        target_mode,
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    
    Ok(relationships)
}
