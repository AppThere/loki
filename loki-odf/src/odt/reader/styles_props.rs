// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Style-property element parsing for the ODT styles reader (split from
//! `styles.rs` for the 300-line ceiling): reads the `style:*-properties`
//! children of a `style:style` / `style:default-style` into a
//! [`ParsedStyleProps`]. `parse_style_props` is re-exported from `styles.rs`;
//! the graphic/cell element builders are private to this module.
// `drop(ref_binding)` is a deliberate NLL-boundary hint (see `styles.rs`).
#![allow(dropping_references)]

use quick_xml::Reader;
use quick_xml::events::Event;

use super::{ParsedStyleProps, para_props, skip_element, table_style};
use crate::error::{OdfError, OdfResult};
use crate::odt::model::styles::{OdfCellProps, OdfGraphicWrap, OdfTextProps};
use crate::xml_util::local_attr_val;

/// Parse the attributes of a `style:text-properties` element into an
/// [`OdfTextProps`]. Shared by the style-props reader and the list-style reader
/// (re-exported from `styles.rs` as `parse_text_props_attrs`).
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
        font_relief: local_attr_val(e, b"font-relief"),
        border: local_attr_val(e, b"border"),
        padding: local_attr_val(e, b"padding"),
        word_spacing: local_attr_val(e, b"word-spacing"),
        letter_kerning: local_attr_val(e, b"letter-kerning").map(|v| v == "true"),
        text_scale: local_attr_val(e, b"text-scale"),
        language_complex: local_attr_val(e, b"language-complex"),
        country_complex: local_attr_val(e, b"country-complex"),
        language_asian: local_attr_val(e, b"language-asian"),
        country_asian: local_attr_val(e, b"country-asian"),
    }
}

/// Read the children of a `style:style` or `style:default-style` element
/// until the matching end tag, collecting every `style:*-properties` child
/// into a [`ParsedStyleProps`].
pub(super) fn parse_style_props(
    reader: &mut Reader<&[u8]>,
    end_local: &[u8],
) -> OdfResult<ParsedStyleProps> {
    let mut buf = Vec::new();
    let mut props = ParsedStyleProps::default();

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
                        props.para_props = Some(pp);
                    }
                    b"text-properties" => {
                        props.text_props = Some(parse_text_props_attrs(e));
                        drop(e);
                        skip_element(reader, b"text-properties")?;
                    }
                    b"table-column-properties" => {
                        props.col_width = crate::xml_util::local_attr_val(e, b"column-width");
                        drop(e);
                        skip_element(reader, b"table-column-properties")?;
                    }
                    // COMPAT(odf): style:table-cell-properties may be self-closing
                    // (Empty) or have children (Start/End); handle both.
                    b"table-cell-properties" => {
                        props.cell_props = Some(parse_cell_props_element(e));
                        drop(e);
                        skip_element(reader, b"table-cell-properties")?;
                    }
                    b"table-properties" => {
                        props.table_props = Some(table_style::parse_table_props_element(e));
                        drop(e);
                        skip_element(reader, b"table-properties")?;
                    }
                    b"graphic-properties" => {
                        props.graphic_wrap = Some(parse_graphic_wrap_element(e));
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
                        props.para_props = Some(para_props::parse_para_props_element(e));
                    }
                    b"text-properties" => {
                        props.text_props = Some(parse_text_props_attrs(e));
                    }
                    b"table-column-properties" => {
                        props.col_width = crate::xml_util::local_attr_val(e, b"column-width");
                    }
                    b"table-cell-properties" => {
                        props.cell_props = Some(parse_cell_props_element(e));
                    }
                    b"table-properties" => {
                        props.table_props = Some(table_style::parse_table_props_element(e));
                    }
                    b"graphic-properties" => {
                        props.graphic_wrap = Some(parse_graphic_wrap_element(e));
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

    Ok(props)
}

/// Build an [`OdfGraphicWrap`] from a `style:graphic-properties` element.
fn parse_graphic_wrap_element(e: &quick_xml::events::BytesStart<'_>) -> OdfGraphicWrap {
    // `draw:fill-color` / `svg:stroke-color` are only meaningful when their
    // matching `draw:fill` / `draw:stroke` is `"solid"`; a `"none"` toggle drops
    // the colour so an unfilled/unstroked frame does not resurrect one.
    let solid = |toggle: &[u8]| local_attr_val(e, toggle).as_deref() != Some("none");
    OdfGraphicWrap {
        wrap: local_attr_val(e, b"wrap"),
        run_through: local_attr_val(e, b"run-through"),
        fill_color: solid(b"fill")
            .then(|| local_attr_val(e, b"fill-color"))
            .flatten(),
        stroke_color: solid(b"stroke")
            .then(|| local_attr_val(e, b"stroke-color"))
            .flatten(),
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
