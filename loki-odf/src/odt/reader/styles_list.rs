// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List-level parsing for the ODT `styles.xml` reader: the
//! `style:list-level-properties` / `style:text-properties` children of a
//! `text:list-level-style-*` element, and its `style:list-level-label-alignment`.
//! Split out of `styles.rs` (Phase 7.1); `parse_list_style` (in `styles.rs`)
//! calls `parse_list_level_props`. ODF 1.3 §16.31–§16.34.

// `drop(ref_binding)` is a deliberate NLL-boundary hint (see `styles.rs`).
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{OdfError, OdfResult};
use crate::odt::model::list_styles::{OdfListLevel, OdfListLevelKind};
use crate::xml_util::local_attr_val;

use super::{parse_text_props_attrs, skip_element};

/// Parse the children of a `text:list-level-style-*` element:
/// `style:list-level-properties` and optionally `style:text-properties`.
pub(super) fn parse_list_level_props(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
    level: u8,
    kind: OdfListLevelKind,
) -> OdfResult<OdfListLevel> {
    let mut buf = Vec::new();
    let mut out = OdfListLevel {
        level,
        kind,
        legacy_space_before: None,
        legacy_min_label_width: None,
        legacy_min_label_distance: None,
        label_followed_by: None,
        list_tab_stop_position: None,
        text_indent: None,
        margin_left: None,
        text_props: None,
    };

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"list-level-properties" => {
                        let mode = local_attr_val(e, b"list-level-position-and-space-mode");
                        if mode.as_deref() == Some("label-alignment") {
                            // read child style:list-level-label-alignment
                            drop(e);
                            parse_label_alignment_child(reader, &mut out)?;
                        } else {
                            // legacy ODF 1.1 attrs directly on the element
                            out.legacy_space_before = local_attr_val(e, b"space-before");
                            out.legacy_min_label_width = local_attr_val(e, b"min-label-width");
                            out.legacy_min_label_distance =
                                local_attr_val(e, b"min-label-distance");
                            drop(e);
                            skip_element(reader, b"list-level-properties")?;
                        }
                    }
                    b"text-properties" => {
                        let tp = parse_text_props_attrs(e);
                        drop(e);
                        skip_element(reader, b"text-properties")?;
                        out.text_props = Some(tp);
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
                    b"list-level-properties" => {
                        let mode = local_attr_val(e, b"list-level-position-and-space-mode");
                        if mode.as_deref() != Some("label-alignment") {
                            out.legacy_space_before = local_attr_val(e, b"space-before");
                            out.legacy_min_label_width = local_attr_val(e, b"min-label-width");
                            out.legacy_min_label_distance =
                                local_attr_val(e, b"min-label-distance");
                        }
                        // label-alignment on an empty element has no child
                        // to read; nothing extra to do.
                    }
                    b"text-properties" => {
                        out.text_props = Some(parse_text_props_attrs(e));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == end_local {
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

    Ok(out)
}

/// The `OdfListLevelKind::Image` (feature 5.4) and 0-based level index for a
/// `<text:list-level-style-image>` element (`xlink:href` + `text:style-name`).
fn image_kind_and_level(e: &BytesStart) -> (OdfListLevelKind, u8) {
    let level = local_attr_val(e, b"level")
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(1)
        .saturating_sub(1);
    let kind = OdfListLevelKind::Image {
        href: local_attr_val(e, b"href").unwrap_or_default(),
        style_name: local_attr_val(e, b"style-name"),
    };
    (kind, level)
}

/// Parse a `<text:list-level-style-image>` element (with children) into an
/// image-bullet level (feature 5.4).
pub(super) fn parse_image_level(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
) -> OdfResult<OdfListLevel> {
    let (kind, level) = image_kind_and_level(e);
    drop(e);
    parse_list_level_props(reader, b"list-level-style-image", level, kind)
}

/// Build an image-bullet level from a self-closing `<text:list-level-style-image/>`.
pub(super) fn image_level_empty(e: &BytesStart) -> OdfListLevel {
    let (kind, level) = image_kind_and_level(e);
    OdfListLevel {
        level,
        kind,
        legacy_space_before: None,
        legacy_min_label_width: None,
        legacy_min_label_distance: None,
        label_followed_by: None,
        list_tab_stop_position: None,
        text_indent: None,
        margin_left: None,
        text_props: None,
    }
}

/// Inside a `style:list-level-properties` with `label-alignment` mode, read
/// the `style:list-level-label-alignment` child element for positioning attrs.
fn parse_label_alignment_child(
    reader: &mut Reader<&[u8]>,
    level: &mut OdfListLevel,
) -> OdfResult<()> {
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"list-level-label-alignment" {
                    level.label_followed_by = local_attr_val(e, b"label-followed-by");
                    level.list_tab_stop_position = local_attr_val(e, b"list-tab-stop-position");
                    level.text_indent = local_attr_val(e, b"text-indent");
                    level.margin_left = local_attr_val(e, b"margin-left");
                }
            }
            Ok(Event::Start(ref e)) => {
                if e.local_name().into_inner() == b"list-level-label-alignment" {
                    level.label_followed_by = local_attr_val(e, b"label-followed-by");
                    level.list_tab_stop_position = local_attr_val(e, b"list-tab-stop-position");
                    level.text_indent = local_attr_val(e, b"text-indent");
                    level.margin_left = local_attr_val(e, b"margin-left");
                    drop(e);
                    skip_element(reader, b"list-level-label-alignment")?;
                } else {
                    let local = e.local_name().into_inner().to_vec();
                    drop(e);
                    skip_element(reader, &local)?;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"list-level-properties" {
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

    Ok(())
}
