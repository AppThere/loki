// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `styles.xml` and the `office:automatic-styles` section of
//! `content.xml`.  Returns an [`OdfStylesheet`].  ODF 1.3 §14–§16.
// Functions in this module are consumed by the document reader added in the
// next session; suppress premature dead-code lints.
#![allow(dead_code)]
// `drop(ref_binding)` is a deliberate NLL-boundary hint and does nothing at
// runtime; silence the compiler's suggestion to use `let _ = ...` instead.
#![allow(dropping_references)]

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::{
    OdfHeaderFooterProps, OdfMasterPage, OdfPageLayout,
};
use crate::odt::model::list_styles::{
    OdfListLevel, OdfListLevelKind, OdfListStyle,
};
use crate::odt::model::paragraph::{OdfParagraph, OdfParagraphChild};
use crate::odt::model::styles::{
    OdfDefaultStyle, OdfParaProps, OdfStyle, OdfStyleFamily, OdfStylesheet,
    OdfTabStop, OdfTextProps,
};
use crate::xml_util::local_attr_val;

// ── Public entry point ─────────────────────────────────────────────────────────

/// Parse `styles.xml` (pass `is_automatic = false`) or the
/// `office:automatic-styles` block from `content.xml`
/// (pass `is_automatic = true`).
///
/// When `is_automatic` is `false` the reader distinguishes between
/// `office:styles` (→ `named_styles`) and `office:automatic-styles`
/// (→ `auto_styles`), and also collects page layouts and master pages.
/// When `is_automatic` is `true` every `style:style` found anywhere in the
/// document goes into `auto_styles`.
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
pub(crate) fn read_stylesheet(
    xml: &[u8],
    is_automatic: bool,
) -> OdfResult<OdfStylesheet> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut sheet = OdfStylesheet::default();

    // Which section we are currently inside (only relevant when !is_automatic).
    let mut in_named = false; // inside office:styles
    let mut in_auto = false; // inside office:automatic-styles
    let mut in_master = false; // inside office:master-styles

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"styles" => {
                        // office:styles (named)
                        in_named = true;
                        in_auto = false;
                        in_master = false;
                    }
                    b"automatic-styles" => {
                        in_named = false;
                        in_auto = true;
                        in_master = false;
                    }
                    b"master-styles" => {
                        in_named = false;
                        in_auto = false;
                        in_master = true;
                    }
                    b"style" => {
                        // style:style
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        let display_name =
                            local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family")
                                .as_deref()
                                .unwrap_or(""),
                        );
                        let parent_name =
                            local_attr_val(e, b"parent-style-name");
                        let list_style_name =
                            local_attr_val(e, b"list-style-name");
                        let auto = is_automatic || in_auto;
                        drop(e);
                        let (para_props, text_props) =
                            parse_style_props(&mut reader, b"style")?;
                        let style = OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props,
                            text_props,
                            is_automatic: auto,
                        };
                        if auto {
                            sheet.auto_styles.push(style);
                        } else {
                            sheet.named_styles.push(style);
                        }
                    }
                    b"default-style" => {
                        let family = parse_style_family(
                            local_attr_val(e, b"family")
                                .as_deref()
                                .unwrap_or(""),
                        );
                        drop(e);
                        let (para_props, text_props) =
                            parse_style_props(&mut reader, b"default-style")?;
                        sheet.default_styles.push(OdfDefaultStyle {
                            family,
                            para_props,
                            text_props,
                        });
                    }
                    b"list-style" => {
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        drop(e);
                        let list_style =
                            parse_list_style(&mut reader, name)?;
                        sheet.list_styles.push(list_style);
                    }
                    b"page-layout" => {
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        drop(e);
                        let layout =
                            parse_page_layout(&mut reader, name)?;
                        sheet.page_layouts.push(layout);
                    }
                    b"master-page" => {
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        let page_layout_name =
                            local_attr_val(e, b"page-layout-name")
                                .unwrap_or_default();
                        drop(e);
                        let master = parse_master_page(
                            &mut reader,
                            name,
                            page_layout_name,
                        )?;
                        sheet.master_pages.push(master);
                    }
                    _ => {
                        // Skip unrecognised elements inside known sections.
                        // We only need to skip if we're not in a section we
                        // manage explicitly above.
                        drop(e);
                        if in_named || in_auto || in_master || is_automatic {
                            // don't skip – the enclosing section handler above
                            // will encounter the End tag and close its section
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"style" => {
                        // style:style with no children (no props)
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        let display_name =
                            local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family")
                                .as_deref()
                                .unwrap_or(""),
                        );
                        let parent_name =
                            local_attr_val(e, b"parent-style-name");
                        let list_style_name =
                            local_attr_val(e, b"list-style-name");
                        let auto = is_automatic || in_auto;
                        let style = OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props: None,
                            text_props: None,
                            is_automatic: auto,
                        };
                        if auto {
                            sheet.auto_styles.push(style);
                        } else {
                            sheet.named_styles.push(style);
                        }
                    }
                    b"list-style" => {
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        sheet.list_styles.push(OdfListStyle {
                            name,
                            levels: Vec::new(),
                        });
                    }
                    b"master-page" => {
                        // Self-closing <style:master-page .../> — no header/footer content.
                        // TODO(odf-master-page): style:master-page-name transitions not implemented.
                        let name = local_attr_val(e, b"name")
                            .unwrap_or_default();
                        let page_layout_name =
                            local_attr_val(e, b"page-layout-name")
                                .unwrap_or_default();
                        sheet.master_pages.push(OdfMasterPage {
                            name,
                            page_layout_name,
                            header: None,
                            footer: None,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name().into_inner();
                match local {
                    b"styles" => in_named = false,
                    b"automatic-styles" => in_auto = false,
                    b"master-styles" => in_master = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(sheet)
}

// ── Style property parsing ─────────────────────────────────────────────────────

/// Read the children of a `style:style` or `style:default-style` element
/// until the matching end tag, collecting `style:paragraph-properties` and
/// `style:text-properties`.
fn parse_style_props(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<(Option<OdfParaProps>, Option<OdfTextProps>)> {
    let mut buf = Vec::new();
    let mut para_props: Option<OdfParaProps> = None;
    let mut text_props: Option<OdfTextProps> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"paragraph-properties" => {
                        let pp = parse_para_props_element(e);
                        drop(e);
                        let pp =
                            parse_para_props_with_children(reader, pp)?;
                        para_props = Some(pp);
                    }
                    b"text-properties" => {
                        let tp = parse_text_props_attrs(e);
                        drop(e);
                        skip_element(reader, b"text-properties")?;
                        text_props = Some(tp);
                    }
                    _ => {
                        let local = local.clone();
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                match local {
                    b"paragraph-properties" => {
                        para_props = Some(parse_para_props_element(e));
                    }
                    b"text-properties" => {
                        text_props = Some(parse_text_props_attrs(e));
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
                })
            }
            _ => {}
        }
    }

    Ok((para_props, text_props))
}

/// Build an [`OdfParaProps`] from the attributes of a
/// `style:paragraph-properties` element.
fn parse_para_props_element(e: &quick_xml::events::BytesStart<'_>) -> OdfParaProps {
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
        widows: local_attr_val(e, b"widows")
            .and_then(|s| s.parse().ok()),
        orphans: local_attr_val(e, b"orphans")
            .and_then(|s| s.parse().ok()),
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
    }
}

/// Continue reading children of `style:paragraph-properties` (for tab stops),
/// returning when the matching end tag is found.
fn parse_para_props_with_children(
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
                    pp.tab_stops =
                        parse_tab_stops(reader)?;
                } else {
                    drop(e);
                    skip_element(reader, &local)?;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"tab-stop" {
                    let position =
                        local_attr_val(e, b"position").unwrap_or_default();
                    let tab_type = local_attr_val(e, b"type");
                    pp.tab_stops.push(OdfTabStop { position, tab_type });
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
                })
            }
            _ => {}
        }
    }

    Ok(pp)
}

/// Parse `style:tab-stops` children until `</style:tab-stops>`.
fn parse_tab_stops(
    reader: &mut Reader<&[u8]>,
) -> OdfResult<Vec<OdfTabStop>> {
    let mut buf = Vec::new();
    let mut stops = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                if e.local_name().into_inner() == b"tab-stop" {
                    let position =
                        local_attr_val(e, b"position").unwrap_or_default();
                    let tab_type = local_attr_val(e, b"type");
                    stops.push(OdfTabStop { position, tab_type });
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
                })
            }
            _ => {}
        }
    }

    Ok(stops)
}

/// Build an [`OdfTextProps`] from the attributes of a
/// `style:text-properties` element.
fn parse_text_props_attrs(e: &quick_xml::events::BytesStart<'_>) -> OdfTextProps {
    OdfTextProps {
        font_name: local_attr_val(e, b"font-name"),
        font_family: local_attr_val(e, b"font-family"),
        font_size: local_attr_val(e, b"font-size"),
        font_weight: local_attr_val(e, b"font-weight"),
        font_style: local_attr_val(e, b"font-style"),
        text_underline_style: local_attr_val(e, b"text-underline-style"),
        text_underline_type: local_attr_val(e, b"text-underline-type"),
        text_line_through_style: local_attr_val(
            e,
            b"text-line-through-style",
        ),
        font_variant: local_attr_val(e, b"font-variant"),
        text_transform: local_attr_val(e, b"text-transform"),
        color: local_attr_val(e, b"color"),
        background_color: local_attr_val(e, b"background-color"),
        text_shadow: local_attr_val(e, b"text-shadow"),
        language: local_attr_val(e, b"language"),
        country: local_attr_val(e, b"country"),
        text_position: local_attr_val(e, b"text-position"),
        letter_spacing: local_attr_val(e, b"letter-spacing"),
        font_size_complex: local_attr_val(e, b"font-size-complex"),
        font_name_complex: local_attr_val(e, b"font-name-complex"),
    }
}

// ── List style parsing ─────────────────────────────────────────────────────────

/// Parse a `text:list-style` element (already consumed Start event).
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
fn parse_list_style(
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
                        let bullet_char =
                            local_attr_val(e, b"bullet-char")
                                .unwrap_or_default();
                        let style_name =
                            local_attr_val(e, b"style-name");
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
                            local_attr_val(e, b"start-value")
                                .and_then(|s| s.parse().ok());
                        let display_levels: u8 =
                            local_attr_val(e, b"display-levels")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        let style_name =
                            local_attr_val(e, b"style-name");
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
                        let bullet_char =
                            local_attr_val(e, b"bullet-char")
                                .unwrap_or_default();
                        let style_name =
                            local_attr_val(e, b"style-name");
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
                            local_attr_val(e, b"start-value")
                                .and_then(|s| s.parse().ok());
                        let display_levels: u8 =
                            local_attr_val(e, b"display-levels")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1);
                        let style_name =
                            local_attr_val(e, b"style-name");
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
                })
            }
            _ => {}
        }
    }

    Ok(OdfListStyle { name, levels })
}

/// Parse the children of a `text:list-level-style-*` element:
/// `style:list-level-properties` and optionally `style:text-properties`.
fn parse_list_level_props(
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
                        let mode = local_attr_val(
                            e,
                            b"list-level-position-and-space-mode",
                        );
                        if mode.as_deref()
                            == Some("label-alignment")
                        {
                            // read child style:list-level-label-alignment
                            drop(e);
                            parse_label_alignment_child(reader, &mut out)?;
                        } else {
                            // legacy ODF 1.1 attrs directly on the element
                            out.legacy_space_before =
                                local_attr_val(e, b"space-before");
                            out.legacy_min_label_width =
                                local_attr_val(e, b"min-label-width");
                            out.legacy_min_label_distance =
                                local_attr_val(e, b"min-label-distance");
                            drop(e);
                            skip_element(
                                reader,
                                b"list-level-properties",
                            )?;
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
                        let mode = local_attr_val(
                            e,
                            b"list-level-position-and-space-mode",
                        );
                        if mode.as_deref() != Some("label-alignment") {
                            out.legacy_space_before =
                                local_attr_val(e, b"space-before");
                            out.legacy_min_label_width =
                                local_attr_val(e, b"min-label-width");
                            out.legacy_min_label_distance =
                                local_attr_val(e, b"min-label-distance");
                        }
                        // label-alignment on an empty element has no child
                        // to read; nothing extra to do.
                    }
                    b"text-properties" => {
                        out.text_props =
                            Some(parse_text_props_attrs(e));
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
                })
            }
            _ => {}
        }
    }

    Ok(out)
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
                if e.local_name().into_inner()
                    == b"list-level-label-alignment"
                {
                    level.label_followed_by =
                        local_attr_val(e, b"label-followed-by");
                    level.list_tab_stop_position =
                        local_attr_val(e, b"list-tab-stop-position");
                    level.text_indent =
                        local_attr_val(e, b"text-indent");
                    level.margin_left =
                        local_attr_val(e, b"margin-left");
                }
            }
            Ok(Event::Start(ref e)) => {
                if e.local_name().into_inner()
                    == b"list-level-label-alignment"
                {
                    level.label_followed_by =
                        local_attr_val(e, b"label-followed-by");
                    level.list_tab_stop_position =
                        local_attr_val(e, b"list-tab-stop-position");
                    level.text_indent =
                        local_attr_val(e, b"text-indent");
                    level.margin_left =
                        local_attr_val(e, b"margin-left");
                    drop(e);
                    skip_element(reader, b"list-level-label-alignment")?;
                } else {
                    let local = e.local_name().into_inner().to_vec();
                    drop(e);
                    skip_element(reader, &local)?;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"list-level-properties"
                {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(())
}

// ── Page layout parsing ────────────────────────────────────────────────────────

/// Parse a `style:page-layout` element (Start event already consumed).
fn parse_page_layout(
    reader: &mut Reader<&[u8]>,
    name: String,
) -> OdfResult<OdfPageLayout> {
    let mut buf = Vec::new();
    let mut layout = OdfPageLayout {
        name,
        page_width: None,
        page_height: None,
        margin_top: None,
        margin_bottom: None,
        margin_left: None,
        margin_right: None,
        print_orientation: None,
        header_props: None,
        footer_props: None,
    };

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"page-layout-properties" => {
                        layout.page_width =
                            local_attr_val(e, b"page-width");
                        layout.page_height =
                            local_attr_val(e, b"page-height");
                        layout.margin_top =
                            local_attr_val(e, b"margin-top");
                        layout.margin_bottom =
                            local_attr_val(e, b"margin-bottom");
                        layout.margin_left =
                            local_attr_val(e, b"margin-left");
                        layout.margin_right =
                            local_attr_val(e, b"margin-right");
                        layout.print_orientation =
                            local_attr_val(e, b"print-orientation");
                        drop(e);
                        skip_element(reader, b"page-layout-properties")?;
                    }
                    b"header-style" => {
                        drop(e);
                        layout.header_props = Some(
                            parse_header_footer_style(reader, b"header-style")?,
                        );
                    }
                    b"footer-style" => {
                        drop(e);
                        layout.footer_props = Some(
                            parse_header_footer_style(reader, b"footer-style")?,
                        );
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name().into_inner();
                if local == b"page-layout-properties" {
                    layout.page_width =
                        local_attr_val(e, b"page-width");
                    layout.page_height =
                        local_attr_val(e, b"page-height");
                    layout.margin_top =
                        local_attr_val(e, b"margin-top");
                    layout.margin_bottom =
                        local_attr_val(e, b"margin-bottom");
                    layout.margin_left =
                        local_attr_val(e, b"margin-left");
                    layout.margin_right =
                        local_attr_val(e, b"margin-right");
                    layout.print_orientation =
                        local_attr_val(e, b"print-orientation");
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"page-layout" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(layout)
}

/// Parse the `style:header-footer-properties` child of a
/// `style:header-style` / `style:footer-style` element.
fn parse_header_footer_style(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<OdfHeaderFooterProps> {
    let mut buf = Vec::new();
    let mut props = OdfHeaderFooterProps {
        min_height: None,
        margin: None,
    };

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                if e.local_name().into_inner()
                    == b"header-footer-properties"
                {
                    props.min_height =
                        local_attr_val(e, b"min-height");
                    props.margin = local_attr_val(e, b"margin");
                    if matches!(
                        reader.read_event_into(&mut buf),
                        Ok(Event::End(_))
                    ) {}
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
                })
            }
            _ => {}
        }
    }

    Ok(props)
}

// ── Master page parsing ────────────────────────────────────────────────────────

/// Parse a `style:master-page` element (Start event already consumed).
fn parse_master_page(
    reader: &mut Reader<&[u8]>,
    name: String,
    page_layout_name: String,
) -> OdfResult<OdfMasterPage> {
    let mut buf = Vec::new();
    let mut header: Option<Vec<OdfParagraph>> = None;
    let mut footer: Option<Vec<OdfParagraph>> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"header" => {
                        drop(e);
                        header = Some(parse_header_footer_paras(
                            reader, b"header",
                        )?);
                    }
                    b"footer" => {
                        drop(e);
                        footer = Some(parse_header_footer_paras(
                            reader, b"footer",
                        )?);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"master-page" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(OdfMasterPage {
        name,
        page_layout_name,
        header,
        footer,
    })
}

/// Collect paragraphs inside a `style:header` or `style:footer` element.
///
/// Uses a simplified reader that only captures `text:style-name` and flat
/// text content — sufficient for header/footer paragraphs without needing the
/// full inline-content parser from `document.rs`.
fn parse_header_footer_paras(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<Vec<OdfParagraph>> {
    let mut buf = Vec::new();
    let mut paras = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"p" | b"h" => {
                        let style = local_attr_val(e, b"style-name");
                        let is_heading = local == b"h";
                        let outline_level: Option<u8> = if is_heading {
                            local_attr_val(e, b"outline-level")
                                .and_then(|s| s.parse().ok())
                        } else {
                            None
                        };
                        let end_tag: &'static [u8] =
                            if is_heading { b"h" } else { b"p" };
                        drop(e);
                        let para = collect_para_text(
                            reader,
                            end_tag,
                            style,
                            is_heading,
                            outline_level,
                        )?;
                        paras.push(para);
                    }
                    _ => {
                        drop(e);
                        skip_element(reader, &local)?;
                    }
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
                })
            }
            _ => {}
        }
    }

    Ok(paras)
}

/// Simplified paragraph reader: collects only text nodes (flat) until the
/// matching end tag. Used for header/footer paragraphs where deep inline
/// parsing is not required.
fn collect_para_text(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
    style_name: Option<String>,
    is_heading: bool,
    outline_level: Option<u8>,
) -> OdfResult<OdfParagraph> {
    let mut buf = Vec::new();
    let mut children: Vec<OdfParagraphChild> = Vec::new();
    let mut depth: u32 = 0;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref t)) => {
                let s = t.unescape().map_err(|e| OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })?;
                if !s.is_empty() {
                    children.push(OdfParagraphChild::Text(s.into_owned()));
                }
            }
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                if depth == 0
                    && e.local_name().into_inner() == end_local
                {
                    break;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(OdfParagraph {
        style_name,
        outline_level,
        is_heading,
        children,
        list_context: None,
    })
}

// ── Auto-styles fast reader (content.xml) ─────────────────────────────────────

/// Parse the `office:automatic-styles` section of `content.xml`, returning
/// only the `style:style` elements found within it, all marked
/// `is_automatic = true`.
///
/// Stops reading as soon as the closing `</office:automatic-styles>` tag is
/// encountered, so the (potentially large) `office:body` section is never
/// touched.
pub(crate) fn read_auto_styles(xml: &[u8]) -> OdfResult<Vec<OdfStyle>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut styles: Vec<OdfStyle> = Vec::new();
    let mut inside = false;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"automatic-styles" => inside = true,
                    b"style" if inside => {
                        let name =
                            local_attr_val(e, b"name").unwrap_or_default();
                        let display_name =
                            local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family")
                                .as_deref()
                                .unwrap_or(""),
                        );
                        let parent_name =
                            local_attr_val(e, b"parent-style-name");
                        let list_style_name =
                            local_attr_val(e, b"list-style-name");
                        drop(e);
                        let (para_props, text_props) =
                            parse_style_props(&mut reader, b"style")?;
                        styles.push(OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props,
                            text_props,
                            is_automatic: true,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                if inside && e.local_name().into_inner() == b"style" {
                    let name =
                        local_attr_val(e, b"name").unwrap_or_default();
                    let display_name = local_attr_val(e, b"display-name");
                    let family = parse_style_family(
                        local_attr_val(e, b"family")
                            .as_deref()
                            .unwrap_or(""),
                    );
                    let parent_name =
                        local_attr_val(e, b"parent-style-name");
                    let list_style_name =
                        local_attr_val(e, b"list-style-name");
                    styles.push(OdfStyle {
                        name,
                        display_name,
                        family,
                        parent_name,
                        list_style_name,
                        para_props: None,
                        text_props: None,
                        is_automatic: true,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"automatic-styles" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }

    Ok(styles)
}

// ── Shared helpers ─────────────────────────────────────────────────────────────

fn parse_style_family(s: &str) -> OdfStyleFamily {
    match s {
        "paragraph" => OdfStyleFamily::Paragraph,
        "text" => OdfStyleFamily::Text,
        "table" => OdfStyleFamily::Table,
        "table-row" => OdfStyleFamily::TableRow,
        "table-cell" => OdfStyleFamily::TableCell,
        "graphic" => OdfStyleFamily::Graphic,
        _ => OdfStyleFamily::Unknown,
    }
}

/// Skip the current open element and all its descendants, stopping after the
/// matching end tag. Assumes the `Start` event has already been consumed.
pub(crate) fn skip_element(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<()> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0
                    && e.local_name().into_inner() == end_local
                {
                    return Ok(());
                }
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "styles.xml".to_string(),
                    source: e,
                })
            }
            _ => {}
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_stylesheet_named_style_with_props() {
        let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <style:style style:name="Text_20_Body" style:family="paragraph"
                 style:display-name="Text Body">
      <style:paragraph-properties fo:margin-top="0.2cm" fo:margin-bottom="0.2cm"/>
      <style:text-properties fo:font-size="12pt" fo:font-weight="bold"/>
    </style:style>
  </office:styles>
</office:document-styles>"#;

        let sheet = read_stylesheet(xml, false).unwrap();
        assert_eq!(sheet.named_styles.len(), 1);
        let s = &sheet.named_styles[0];
        assert_eq!(s.name, "Text_20_Body");
        assert_eq!(s.display_name.as_deref(), Some("Text Body"));
        assert_eq!(s.family, OdfStyleFamily::Paragraph);
        assert!(!s.is_automatic);

        let pp = s.para_props.as_ref().unwrap();
        assert_eq!(pp.margin_top.as_deref(), Some("0.2cm"));
        assert_eq!(pp.margin_bottom.as_deref(), Some("0.2cm"));

        let tp = s.text_props.as_ref().unwrap();
        assert_eq!(tp.font_size.as_deref(), Some("12pt"));
        assert_eq!(tp.font_weight.as_deref(), Some("bold"));
    }

    #[test]
    fn read_stylesheet_list_style_bullet_level() {
        let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="List_Bullet">
      <text:list-level-style-bullet text:level="1" text:bullet-char="-">
        <style:list-level-properties
            text:list-level-position-and-space-mode="label-alignment">
          <style:list-level-label-alignment
              text:label-followed-by="listtab"
              text:list-tab-stop-position="0.635cm"
              fo:text-indent="-0.635cm"
              fo:margin-left="0.635cm"/>
        </style:list-level-properties>
      </text:list-level-style-bullet>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

        let sheet = read_stylesheet(xml, false).unwrap();
        assert_eq!(sheet.list_styles.len(), 1);
        let ls = &sheet.list_styles[0];
        assert_eq!(ls.name, "List_Bullet");
        assert_eq!(ls.levels.len(), 1);

        let lv = &ls.levels[0];
        assert_eq!(lv.level, 0);
        assert!(matches!(lv.kind, OdfListLevelKind::Bullet { ref char, .. } if char == "-"));
        assert_eq!(lv.label_followed_by.as_deref(), Some("listtab"));
        assert_eq!(lv.list_tab_stop_position.as_deref(), Some("0.635cm"));
        assert_eq!(lv.text_indent.as_deref(), Some("-0.635cm"));
        assert_eq!(lv.margin_left.as_deref(), Some("0.635cm"));
    }

    #[test]
    fn read_stylesheet_list_style_number_label_alignment() {
        let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="Numbered_List">
      <text:list-level-style-number text:level="1"
          style:num-format="1" style:num-suffix="." text:start-value="1">
        <style:list-level-properties
            text:list-level-position-and-space-mode="label-alignment">
          <style:list-level-label-alignment
              text:label-followed-by="listtab"
              text:list-tab-stop-position="1.27cm"
              fo:text-indent="-0.635cm"
              fo:margin-left="1.27cm"/>
        </style:list-level-properties>
      </text:list-level-style-number>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

        let sheet = read_stylesheet(xml, false).unwrap();
        let ls = &sheet.list_styles[0];
        let lv = &ls.levels[0];
        assert!(
            matches!(lv.kind, OdfListLevelKind::Number { ref num_format, .. }
                if num_format.as_deref() == Some("1"))
        );
        assert_eq!(lv.label_followed_by.as_deref(), Some("listtab"));
        assert_eq!(lv.margin_left.as_deref(), Some("1.27cm"));
    }

    #[test]
    fn read_auto_styles_returns_automatic_styles() {
        let xml = br#"<?xml version="1.0"?>
<office:document-content
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
    office:version="1.3">
  <office:automatic-styles>
    <style:style style:name="P1" style:family="paragraph"
                 style:parent-style-name="Default_20_Paragraph_20_Style">
      <style:paragraph-properties fo:margin-left="0.5cm"/>
      <style:text-properties fo:font-size="10pt"/>
    </style:style>
    <style:style style:name="T1" style:family="text"/>
  </office:automatic-styles>
  <office:body>
    <office:text/>
  </office:body>
</office:document-content>"#;

        let styles = read_auto_styles(xml).unwrap();
        assert_eq!(styles.len(), 2);

        let p1 = &styles[0];
        assert_eq!(p1.name, "P1");
        assert_eq!(p1.family, OdfStyleFamily::Paragraph);
        assert!(p1.is_automatic);
        assert_eq!(
            p1.parent_name.as_deref(),
            Some("Default_20_Paragraph_20_Style")
        );
        let pp = p1.para_props.as_ref().unwrap();
        assert_eq!(pp.margin_left.as_deref(), Some("0.5cm"));
        let tp = p1.text_props.as_ref().unwrap();
        assert_eq!(tp.font_size.as_deref(), Some("10pt"));

        let t1 = &styles[1];
        assert_eq!(t1.name, "T1");
        assert_eq!(t1.family, OdfStyleFamily::Text);
        assert!(t1.is_automatic);
        assert!(t1.para_props.is_none());
    }

    #[test]
    fn read_stylesheet_list_style_legacy_positioning() {
        let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="ODF11_List">
      <text:list-level-style-bullet text:level="1" text:bullet-char="-">
        <style:list-level-properties
            text:space-before="0.25cm"
            text:min-label-width="0.4cm"
            text:min-label-distance="0.1cm"/>
      </text:list-level-style-bullet>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

        let sheet = read_stylesheet(xml, false).unwrap();
        let lv = &sheet.list_styles[0].levels[0];
        assert_eq!(lv.legacy_space_before.as_deref(), Some("0.25cm"));
        assert_eq!(lv.legacy_min_label_width.as_deref(), Some("0.4cm"));
        assert_eq!(lv.legacy_min_label_distance.as_deref(), Some("0.1cm"));
        assert!(lv.label_followed_by.is_none());
    }
}
