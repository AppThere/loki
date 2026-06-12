// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsing of `style:style`, `style:table-cell-properties`, and
//! `style:text-properties` elements.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::styles::{OdfCellProps, OdfParaProps, OdfStyleFamily, OdfTextProps};
use crate::xml_util::local_attr_val;

use super::para_props::{parse_para_props_element, parse_para_props_with_children};
use super::util::skip_element;

/// Map an ODF style family string to [`OdfStyleFamily`].
pub(crate) fn parse_style_family(s: &str) -> OdfStyleFamily {
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

/// Read the children of a `style:style` or `style:default-style` element
/// until the matching end tag, collecting `style:paragraph-properties`,
/// `style:text-properties`, `style:table-column-properties`, and
/// `style:table-cell-properties`.
///
/// Returns `(para_props, text_props, col_width, cell_props)`.
#[allow(clippy::type_complexity)] // Pre-existing pattern — structural refactor deferred
pub(crate) fn parse_style_props(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<(
    Option<OdfParaProps>,
    Option<OdfTextProps>,
    Option<String>,
    Option<OdfCellProps>,
)> {
    let mut buf = Vec::new();
    let mut para_props: Option<OdfParaProps> = None;
    let mut text_props: Option<OdfTextProps> = None;
    let mut col_width: Option<String> = None;
    let mut cell_props: Option<OdfCellProps> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"paragraph-properties" => {
                        let pp = parse_para_props_element(e);
                        drop(e);
                        let pp = parse_para_props_with_children(reader, pp)?;
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
                    b"table-column-properties" => {
                        col_width = crate::xml_util::local_attr_val(e, b"column-width");
                    }
                    b"table-cell-properties" => {
                        cell_props = Some(parse_cell_props_element(e));
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

    Ok((para_props, text_props, col_width, cell_props))
}

/// Build an [`OdfCellProps`] from the attributes of a
/// `style:table-cell-properties` element.
///
/// ODF shorthand `fo:padding` sets all four edges; individual edge attributes
/// (`fo:padding-top` etc.) take precedence over the shorthand.
/// Same logic applies to `fo:border` vs per-edge border attributes.
pub(super) fn parse_cell_props_element(e: &quick_xml::events::BytesStart<'_>) -> OdfCellProps {
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
