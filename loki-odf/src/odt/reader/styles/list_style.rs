// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsing of `text:list-style` and related list-level elements.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::list_styles::{OdfListLevel, OdfListLevelKind, OdfListStyle};
use crate::xml_util::local_attr_val;

use super::list_level::parse_list_level_props;
use super::util::skip_element;

/// Parse a `text:list-style` element (already consumed Start event).
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
pub(super) fn parse_list_style(
    reader: &mut Reader<&[u8]>,
    name: String,
) -> OdfResult<OdfListStyle> {
    let mut buf = Vec::new();
    let mut levels: Vec<OdfListLevel> = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"list-level-style-bullet" => {
                        let level_num: u8 = local_attr_val(e, b"level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let bullet_char = local_attr_val(e, b"bullet-char").unwrap_or_default();
                        let style_name = local_attr_val(e, b"style-name");
                        drop(e);
                        let level = parse_list_level_props(
                            reader,
                            b"list-level-style-bullet",
                            level_num.saturating_sub(1),
                            OdfListLevelKind::Bullet {
                                char: bullet_char,
                                style_name,
                            },
                        )?;
                        levels.push(level);
                    }
                    b"list-level-style-number" => {
                        let level_num: u8 = local_attr_val(e, b"level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let num_format = local_attr_val(e, b"num-format");
                        let num_prefix = local_attr_val(e, b"num-prefix");
                        let num_suffix = local_attr_val(e, b"num-suffix");
                        let start_value: Option<u32> =
                            local_attr_val(e, b"start-value").and_then(|s| s.parse().ok());
                        let display_levels: u8 = local_attr_val(e, b"display-levels")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let style_name = local_attr_val(e, b"style-name");
                        drop(e);
                        let level = parse_list_level_props(
                            reader,
                            b"list-level-style-number",
                            level_num.saturating_sub(1),
                            OdfListLevelKind::Number {
                                num_format,
                                num_prefix,
                                num_suffix,
                                start_value,
                                display_levels,
                                style_name,
                            },
                        )?;
                        levels.push(level);
                    }
                    b"list-level-style-none" => {
                        let level_num: u8 = local_attr_val(e, b"level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        drop(e);
                        let level = parse_list_level_props(
                            reader,
                            b"list-level-style-none",
                            level_num.saturating_sub(1),
                            OdfListLevelKind::None,
                        )?;
                        levels.push(level);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"list-level-style-bullet" => {
                        let level_num: u8 = local_attr_val(e, b"level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let bullet_char = local_attr_val(e, b"bullet-char").unwrap_or_default();
                        let style_name = local_attr_val(e, b"style-name");
                        levels.push(OdfListLevel {
                            level: level_num.saturating_sub(1),
                            kind: OdfListLevelKind::Bullet {
                                char: bullet_char,
                                style_name,
                            },
                            legacy_space_before: None,
                            legacy_min_label_width: None,
                            legacy_min_label_distance: None,
                            label_followed_by: None,
                            list_tab_stop_position: None,
                            text_indent: None,
                            margin_left: None,
                            text_props: None,
                        });
                    }
                    b"list-level-style-number" => {
                        let level_num: u8 = local_attr_val(e, b"level")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let num_format = local_attr_val(e, b"num-format");
                        let num_prefix = local_attr_val(e, b"num-prefix");
                        let num_suffix = local_attr_val(e, b"num-suffix");
                        let start_value: Option<u32> =
                            local_attr_val(e, b"start-value").and_then(|s| s.parse().ok());
                        let display_levels: u8 = local_attr_val(e, b"display-levels")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let style_name = local_attr_val(e, b"style-name");
                        levels.push(OdfListLevel {
                            level: level_num.saturating_sub(1),
                            kind: OdfListLevelKind::Number {
                                num_format,
                                num_prefix,
                                num_suffix,
                                start_value,
                                display_levels,
                                style_name,
                            },
                            legacy_space_before: None,
                            legacy_min_label_width: None,
                            legacy_min_label_distance: None,
                            label_followed_by: None,
                            list_tab_stop_position: None,
                            text_indent: None,
                            margin_left: None,
                            text_props: None,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"list-style" {
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

    Ok(OdfListStyle { name, levels })
}
