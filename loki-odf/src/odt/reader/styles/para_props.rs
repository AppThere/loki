// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsing of `style:paragraph-properties` and `style:tab-stops`.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::styles::{OdfParaProps, OdfTabStop};
use crate::xml_util::local_attr_val;

use super::util::skip_element;

/// Build an [`OdfParaProps`] from the attributes of a
/// `style:paragraph-properties` element.
pub(super) fn parse_para_props_element(e: &quick_xml::events::BytesStart<'_>) -> OdfParaProps {
    OdfParaProps {
        margin_top: local_attr_val(e, b"margin-top"),
        margin_bottom: local_attr_val(e, b"margin-bottom"),
        margin_left: local_attr_val(e, b"margin-left"),
        margin_right: local_attr_val(e, b"margin-right"),
        text_indent: local_attr_val(e, b"text-indent"),
        line_height: local_attr_val(e, b"line-height"),
        line_height_at_least: local_attr_val(e, b"line-height-at-least"),
        text_align: local_attr_val(e, b"text-align"),
        keep_together: local_attr_val(e, b"keep-together"),
        keep_with_next: local_attr_val(e, b"keep-with-next"),
        widows: local_attr_val(e, b"widows").and_then(|s| s.parse().ok()),
        orphans: local_attr_val(e, b"orphans").and_then(|s| s.parse().ok()),
        break_before: local_attr_val(e, b"break-before"),
        break_after: local_attr_val(e, b"break-after"),
        border: local_attr_val(e, b"border"),
        border_top: local_attr_val(e, b"border-top"),
        border_bottom: local_attr_val(e, b"border-bottom"),
        border_left: local_attr_val(e, b"border-left"),
        border_right: local_attr_val(e, b"border-right"),
        padding: local_attr_val(e, b"padding"),
        background_color: local_attr_val(e, b"background-color"),
        tab_stops: Vec::new(),
        writing_mode: local_attr_val(e, b"writing-mode"),
    }
}

/// Continue reading children of `style:paragraph-properties` (for tab stops),
/// returning when the matching end tag is found.
pub(super) fn parse_para_props_with_children(
    reader: &mut Reader<&[u8]>,
    mut pp: OdfParaProps,
) -> OdfResult<OdfParaProps> {
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                if local == b"tab-stops" {
                    drop(e);
                    pp.tab_stops = parse_tab_stops(reader)?;
                } else {
                    drop(e);
                    skip_element(reader, &local)?;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"tab-stop" {
                    let position = local_attr_val(e, b"position").unwrap_or_default();
                    let tab_type = local_attr_val(e, b"type");
                    let leader_style = local_attr_val(e, b"leader-style");
                    pp.tab_stops.push(OdfTabStop {
                        position,
                        tab_type,
                        leader_style,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"paragraph-properties" {
                    break;
                }
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

    Ok(pp)
}

/// Parse `style:tab-stops` children until `</style:tab-stops>`.
pub(super) fn parse_tab_stops(reader: &mut Reader<&[u8]>) -> OdfResult<Vec<OdfTabStop>> {
    let mut buf = Vec::new();
    let mut stops = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"tab-stop" {
                    let position = local_attr_val(e, b"position").unwrap_or_default();
                    let tab_type = local_attr_val(e, b"type");
                    let leader_style = local_attr_val(e, b"leader-style");
                    stops.push(OdfTabStop {
                        position,
                        tab_type,
                        leader_style,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"tab-stops" {
                    break;
                }
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

    Ok(stops)
}
