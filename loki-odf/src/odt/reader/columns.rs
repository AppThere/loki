// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for the `style:columns` child of `style:page-layout-properties`
//! (ODF 1.3 §16.27.10) — multi-column section layout.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::OdfColumns;
use crate::odt::reader::styles::skip_element;
use crate::xml_util::local_attr_val;

/// Scans the children of `style:page-layout-properties` for a `style:columns`
/// element, returning the parsed [`OdfColumns`] if present. Consumes up to and
/// including the closing `</style:page-layout-properties>`.
pub(crate) fn parse_plp_columns(reader: &mut Reader<&[u8]>) -> OdfResult<Option<OdfColumns>> {
    let mut buf = Vec::new();
    let mut columns = None;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                if local == b"columns" {
                    columns = Some(parse_columns(reader, e, false)?);
                } else {
                    skip_element(reader, &local)?;
                }
            }
            Ok(Event::Empty(ref e)) if e.local_name().into_inner() == b"columns" => {
                columns = Some(parse_columns(reader, e, true)?);
            }
            Ok(Event::End(ref e)) if e.local_name().into_inner() == b"page-layout-properties" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }
    Ok(columns)
}

/// Parses a `style:columns` element. `count`/`gap` come from its attributes;
/// `separator` is set when a `style:column-sep` child is present. When `empty`
/// the element is self-closing (no separator, nothing more to read).
fn parse_columns(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart<'_>,
    empty: bool,
) -> OdfResult<OdfColumns> {
    let count = local_attr_val(e, b"column-count")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let gap = local_attr_val(e, b"column-gap");
    let mut separator = false;
    let mut rel_widths = Vec::new();
    if !empty {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref c) | Event::Empty(ref c)) => {
                    match c.local_name().into_inner() {
                        b"column-sep" => separator = true,
                        // `style:column @style:rel-width` carries the `"N*"` share
                        // for unequal columns; one per column in order.
                        b"column" => {
                            if let Some(w) = local_attr_val(c, b"rel-width")
                                .and_then(|v| v.trim_end_matches('*').parse::<f32>().ok())
                            {
                                rel_widths.push(w);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref c)) if c.local_name().into_inner() == b"columns" => break,
                Ok(Event::Eof) => break,
                Err(err) => {
                    return Err(OdfError::Xml {
                        part: "styles.xml".to_string(),
                        source: err,
                    });
                }
                _ => {}
            }
        }
    }
    Ok(OdfColumns {
        count,
        gap,
        separator,
        rel_widths,
    })
}
