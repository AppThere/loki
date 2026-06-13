// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parses `ppt/presentation.xml`: slide size and the ordered slide-id list.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::xml_util::{local_attr_val, local_name};

/// Slide size (EMU) and the ordered relationship ids of the slides.
pub(super) struct PresentationInfo {
    /// Slide width in EMU.
    pub width_emu: i64,
    /// Slide height in EMU.
    pub height_emu: i64,
    /// Relationship ids (`r:id`) of the slides, in presentation order.
    pub slide_rids: Vec<String>,
}

/// Parses `presentation.xml` bytes.
pub(super) fn parse_presentation(bytes: &[u8]) -> Result<PresentationInfo, quick_xml::Error> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);

    // Default to 16:9 widescreen (12 192 000 × 6 858 000 EMU) if absent.
    let mut width_emu = 12_192_000;
    let mut height_emu = 6_858_000;
    let mut slide_rids = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => match local_name(&e) {
                b"sldSz" => {
                    if let Some(cx) = local_attr_val(&e, b"cx").and_then(|v| v.parse().ok()) {
                        width_emu = cx;
                    }
                    if let Some(cy) = local_attr_val(&e, b"cy").and_then(|v| v.parse().ok()) {
                        height_emu = cy;
                    }
                }
                // <p:sldId id="256" r:id="rId2"/> — the relationship is r:id, not
                // the bare id; local-name matching would confuse the two.
                b"sldId" => {
                    if let Some(rid) = rel_id_attr(&e) {
                        slide_rids.push(rid);
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(PresentationInfo {
        width_emu,
        height_emu,
        slide_rids,
    })
}

/// Finds the `r:id` attribute (relationships namespace), conventionally bound to
/// the `r` prefix.
fn rel_id_attr(e: &BytesStart<'_>) -> Option<String> {
    e.attributes().flatten().find_map(|attr| {
        if attr.key.as_ref() == b"r:id" {
            attr.unescape_value().ok().map(std::borrow::Cow::into_owned)
        } else {
            None
        }
    })
}
