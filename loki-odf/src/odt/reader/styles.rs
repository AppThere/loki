// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `styles.xml` and the `office:automatic-styles` section of
//! `content.xml`.  Returns an [`OdfStylesheet`].  ODF 1.3 §14–§16.
// Functions in this module are consumed by the document reader added in the
// next session; suppress premature dead-code lints.
#![allow(dead_code)]
// `drop(ref_binding)` is a deliberate NLL-boundary hint and does nothing at
// runtime; silence the compiler's suggestion to use `let _ = ...` instead.
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::document::OdfMasterPage;
use crate::odt::model::list_styles::{OdfListLevel, OdfListLevelKind, OdfListStyle};
use crate::odt::model::styles::{
    OdfCellProps, OdfDefaultStyle, OdfGraphicWrap, OdfParaProps, OdfStyle, OdfStyleFamily,
    OdfStylesheet, OdfTextProps,
};
use crate::xml_util::local_attr_val;

#[path = "styles_list.rs"]
mod list;
#[path = "styles_page.rs"]
mod page;
#[path = "styles_para.rs"]
mod para_props;

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
pub(crate) fn read_stylesheet(xml: &[u8], is_automatic: bool) -> OdfResult<OdfStylesheet> {
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
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        // COMPAT(odf): style:master-page-name on a paragraph
                        // style signals a master page transition. The new master
                        // page's layout applies from that paragraph onward.
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        let auto = is_automatic || in_auto;
                        drop(e);
                        let (para_props, text_props, col_width, cell_props, graphic_wrap) =
                            parse_style_props(&mut reader, b"style")?;
                        let style = OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props,
                            text_props,
                            col_width,
                            cell_props,
                            graphic_wrap,
                            is_automatic: auto,
                            master_page_name,
                        };
                        if auto {
                            sheet.auto_styles.push(style);
                        } else {
                            sheet.named_styles.push(style);
                        }
                    }
                    b"default-style" => {
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        drop(e);
                        let (para_props, text_props, _col_width, _cell_props, _graphic_wrap) =
                            parse_style_props(&mut reader, b"default-style")?;
                        sheet.default_styles.push(OdfDefaultStyle {
                            family,
                            para_props,
                            text_props,
                        });
                    }
                    b"list-style" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        let list_style = parse_list_style(&mut reader, name)?;
                        sheet.list_styles.push(list_style);
                    }
                    b"page-layout" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        let layout = page::parse_page_layout(&mut reader, name)?;
                        sheet.page_layouts.push(layout);
                    }
                    b"master-page" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let page_layout_name =
                            local_attr_val(e, b"page-layout-name").unwrap_or_default();
                        drop(e);
                        let master = page::parse_master_page(
                            &mut reader,
                            name,
                            display_name,
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
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        let auto = is_automatic || in_auto;
                        let style = OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props: None,
                            text_props: None,
                            col_width: None,
                            cell_props: None,
                            graphic_wrap: None,
                            is_automatic: auto,
                            master_page_name,
                        };
                        if auto {
                            sheet.auto_styles.push(style);
                        } else {
                            sheet.named_styles.push(style);
                        }
                    }
                    b"list-style" => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        sheet.list_styles.push(OdfListStyle {
                            name,
                            levels: Vec::new(),
                        });
                    }
                    b"master-page" => {
                        // Self-closing <style:master-page .../> — no header/footer content.
                        // TODO(odf-master-page): style:master-page-name transitions not implemented.
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let page_layout_name =
                            local_attr_val(e, b"page-layout-name").unwrap_or_default();
                        sheet.master_pages.push(OdfMasterPage::header_footer_less(
                            name,
                            display_name,
                            page_layout_name,
                        ));
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
                });
            }
            _ => {}
        }
    }

    Ok(sheet)
}

// ── Style property parsing ─────────────────────────────────────────────────────

/// Read the children of a `style:style` or `style:default-style` element
/// until the matching end tag, collecting `style:paragraph-properties`,
/// `style:text-properties`, `style:table-column-properties`, and
/// `style:table-cell-properties`.
///
/// Returns `(para_props, text_props, col_width, cell_props)`.
#[allow(clippy::type_complexity)] // Pre-existing pattern — structural refactor deferred
fn parse_style_props(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<(
    Option<OdfParaProps>,
    Option<OdfTextProps>,
    Option<String>,
    Option<OdfCellProps>,
    Option<OdfGraphicWrap>,
)> {
    let mut buf = Vec::new();
    let mut para_props: Option<OdfParaProps> = None;
    let mut text_props: Option<OdfTextProps> = None;
    let mut col_width: Option<String> = None;
    let mut cell_props: Option<OdfCellProps> = None;
    let mut graphic_wrap: Option<OdfGraphicWrap> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"paragraph-properties" => {
                        let pp = para_props::parse_para_props_element(e);
                        drop(e);
                        let pp = para_props::parse_para_props_with_children(reader, pp)?;
                        para_props = Some(pp);
                    }
                    b"text-properties" => {
                        let tp = parse_text_props_attrs(e);
                        drop(e);
                        skip_element(reader, b"text-properties")?;
                        text_props = Some(tp);
                    }
                    b"table-column-properties" => {
                        col_width = crate::xml_util::local_attr_val(e, b"column-width");
                        drop(e);
                        skip_element(reader, b"table-column-properties")?;
                    }
                    // COMPAT(odf): style:table-cell-properties may appear as
                    // either a self-closing element (Empty event) or with child
                    // elements (Start/End). Most producers use the self-closing
                    // form, but handle the Start form for robustness.
                    b"table-cell-properties" => {
                        cell_props = Some(parse_cell_props_element(e));
                        drop(e);
                        skip_element(reader, b"table-cell-properties")?;
                    }
                    b"graphic-properties" => {
                        graphic_wrap = Some(parse_graphic_wrap_element(e));
                        drop(e);
                        skip_element(reader, b"graphic-properties")?;
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
                        para_props = Some(para_props::parse_para_props_element(e));
                    }
                    b"text-properties" => {
                        text_props = Some(parse_text_props_attrs(e));
                    }
                    b"table-column-properties" => {
                        col_width = crate::xml_util::local_attr_val(e, b"column-width");
                    }
                    b"table-cell-properties" => {
                        cell_props = Some(parse_cell_props_element(e));
                    }
                    b"graphic-properties" => {
                        graphic_wrap = Some(parse_graphic_wrap_element(e));
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

    Ok((para_props, text_props, col_width, cell_props, graphic_wrap))
}

/// Build an [`OdfGraphicWrap`] from a `style:graphic-properties` element.
fn parse_graphic_wrap_element(e: &quick_xml::events::BytesStart<'_>) -> OdfGraphicWrap {
    OdfGraphicWrap {
        wrap: local_attr_val(e, b"wrap"),
        run_through: local_attr_val(e, b"run-through"),
    }
}

/// Build an [`OdfCellProps`] from the attributes of a
/// `style:table-cell-properties` element.
///
/// ODF shorthand `fo:padding` sets all four edges; individual edge attributes
/// (`fo:padding-top` etc.) take precedence over the shorthand.
/// Same logic applies to `fo:border` vs per-edge border attributes.
fn parse_cell_props_element(e: &quick_xml::events::BytesStart<'_>) -> OdfCellProps {
    // Apply fo:padding shorthand to all edges first.
    let padding_all = local_attr_val(e, b"padding");
    let mut props = OdfCellProps {
        padding_top: padding_all.clone(),
        padding_bottom: padding_all.clone(),
        padding_left: padding_all.clone(),
        padding_right: padding_all,
        vertical_align: local_attr_val(e, b"vertical-align"),
        writing_mode: local_attr_val(e, b"writing-mode"),
        background_color: local_attr_val(e, b"background-color"),
        ..Default::default()
    };
    // Per-edge padding overrides shorthand.
    if let Some(v) = local_attr_val(e, b"padding-top") {
        props.padding_top = Some(v);
    }
    if let Some(v) = local_attr_val(e, b"padding-bottom") {
        props.padding_bottom = Some(v);
    }
    if let Some(v) = local_attr_val(e, b"padding-left") {
        props.padding_left = Some(v);
    }
    if let Some(v) = local_attr_val(e, b"padding-right") {
        props.padding_right = Some(v);
    }

    // Apply fo:border shorthand to all edges first.
    let border_all = local_attr_val(e, b"border");
    props.border_top.clone_from(&border_all);
    props.border_bottom.clone_from(&border_all);
    props.border_left.clone_from(&border_all);
    props.border_right = border_all;
    // Per-edge border overrides shorthand.
    if let Some(v) = local_attr_val(e, b"border-top") {
        props.border_top = Some(v);
    }
    if let Some(v) = local_attr_val(e, b"border-bottom") {
        props.border_bottom = Some(v);
    }
    if let Some(v) = local_attr_val(e, b"border-left") {
        props.border_left = Some(v);
    }
    if let Some(v) = local_attr_val(e, b"border-right") {
        props.border_right = Some(v);
    }

    props
}

/// Build an [`OdfTextProps`] from the attributes of a
/// `style:text-properties` element.
pub(super) fn parse_text_props_attrs(e: &quick_xml::events::BytesStart<'_>) -> OdfTextProps {
    OdfTextProps {
        font_name: local_attr_val(e, b"font-name"),
        font_family: local_attr_val(e, b"font-family"),
        font_size: local_attr_val(e, b"font-size"),
        font_weight: local_attr_val(e, b"font-weight"),
        font_style: local_attr_val(e, b"font-style"),
        text_underline_style: local_attr_val(e, b"text-underline-style"),
        text_underline_type: local_attr_val(e, b"text-underline-type"),
        text_line_through_style: local_attr_val(e, b"text-line-through-style"),
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
        font_name_asian: local_attr_val(e, b"font-name-asian"),
        text_outline: local_attr_val(e, b"text-outline").map(|v| v != "false"),
        word_spacing: local_attr_val(e, b"word-spacing"),
        letter_kerning: local_attr_val(e, b"letter-kerning").map(|v| v == "true"),
        text_scale: local_attr_val(e, b"text-scale"),
        language_complex: local_attr_val(e, b"language-complex"),
        country_complex: local_attr_val(e, b"country-complex"),
        language_asian: local_attr_val(e, b"language-asian"),
        country_asian: local_attr_val(e, b"country-asian"),
    }
}

// ── List style parsing ─────────────────────────────────────────────────────────

/// Parse a `text:list-style` element (already consumed Start event).
#[allow(clippy::too_many_lines)]
// Function body is a single large match over XML events; splitting would reduce readability.
fn parse_list_style(reader: &mut Reader<&[u8]>, name: String) -> OdfResult<OdfListStyle> {
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
                        let level = list::parse_list_level_props(
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
                        let level = list::parse_list_level_props(
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
                        let level = list::parse_list_level_props(
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
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        drop(e);
                        let (para_props, text_props, col_width, cell_props, graphic_wrap) =
                            parse_style_props(&mut reader, b"style")?;
                        styles.push(OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            list_style_name,
                            para_props,
                            text_props,
                            col_width,
                            cell_props,
                            graphic_wrap,
                            is_automatic: true,
                            master_page_name,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                if inside && e.local_name().into_inner() == b"style" {
                    let name = local_attr_val(e, b"name").unwrap_or_default();
                    let display_name = local_attr_val(e, b"display-name");
                    let family =
                        parse_style_family(local_attr_val(e, b"family").as_deref().unwrap_or(""));
                    let parent_name = local_attr_val(e, b"parent-style-name");
                    let list_style_name = local_attr_val(e, b"list-style-name");
                    let master_page_name = local_attr_val(e, b"master-page-name");
                    styles.push(OdfStyle {
                        name,
                        display_name,
                        family,
                        parent_name,
                        list_style_name,
                        para_props: None,
                        text_props: None,
                        col_width: None,
                        cell_props: None,
                        graphic_wrap: None,
                        is_automatic: true,
                        master_page_name,
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
                });
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
pub(crate) fn skip_element(reader: &mut Reader<&[u8]>, end_local: &[u8]) -> OdfResult<()> {
    let mut buf = Vec::new();
    let mut depth: u32 = 1;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0 && e.local_name().into_inner() == end_local {
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
                });
            }
            _ => {}
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "styles_tests.rs"]
mod tests;
