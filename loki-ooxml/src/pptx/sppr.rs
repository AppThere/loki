// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parses a `DrawingML` shape-properties element (`p:spPr`): transform
//! (`a:xfrm`), preset geometry, solid fill, and line stroke.

use loki_graphics::{Fill, PresetShape, Stroke};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::io::BufRead;

use super::units::{color_from_srgb, default_black, emu_to_pt, parse_bool, preset_from_prst};
use super::{end_is, parse_solid_fill_color, skip_subtree};
use crate::xml_util::{local_attr_val, local_name};

/// A parsed `a:xfrm` (offset/extent in EMU, rotation in 60 000ths of a degree).
#[derive(Default)]
pub(super) struct Xfrm {
    pub x_emu: i64,
    pub y_emu: i64,
    pub cx_emu: i64,
    pub cy_emu: i64,
    pub rot: i64,
    pub flip_h: bool,
    pub flip_v: bool,
}

/// The visual properties parsed from a `p:spPr`.
pub(super) struct ShapeProps {
    pub xfrm: Option<Xfrm>,
    pub preset: PresetShape,
    pub fill: Fill,
    pub stroke: Option<Stroke>,
}

impl Default for ShapeProps {
    fn default() -> Self {
        Self {
            xfrm: None,
            preset: PresetShape::Rectangle,
            fill: Fill::None,
            stroke: None,
        }
    }
}

/// Parses a `p:spPr`. Assumes the opening tag was already read; consumes through
/// its end tag.
pub(super) fn parse_sppr<R: BufRead>(
    reader: &mut Reader<R>,
) -> Result<ShapeProps, quick_xml::Error> {
    let mut props = ShapeProps::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"xfrm" => {
                    let mut xfrm = read_xfrm_attrs(&e);
                    read_xfrm_children(reader, &mut xfrm)?;
                    props.xfrm = Some(xfrm);
                }
                b"prstGeom" => {
                    if let Some(prst) = local_attr_val(&e, b"prst") {
                        props.preset = preset_from_prst(&prst);
                    }
                    if matches!(props.preset, PresetShape::RoundedRectangle { .. }) {
                        parse_avlst_adj(reader, &mut props.preset)?;
                    } else {
                        skip_subtree(reader)?;
                    }
                }
                b"solidFill" => {
                    props.fill = parse_solid_fill_color(reader)?.map_or(Fill::None, Fill::Solid);
                }
                b"ln" => {
                    props.stroke = parse_ln(reader, &e)?;
                }
                _ => skip_subtree(reader)?,
            },
            Event::Empty(e) => match local_name(&e) {
                b"xfrm" => props.xfrm = Some(read_xfrm_attrs(&e)),
                b"noFill" => props.fill = Fill::None,
                _ => {}
            },
            Event::End(e) if end_is(&e, b"spPr") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(props)
}

fn read_xfrm_attrs(e: &BytesStart<'_>) -> Xfrm {
    let mut xfrm = Xfrm::default();
    if let Some(r) = local_attr_val(e, b"rot") {
        xfrm.rot = r.parse().unwrap_or(0);
    }
    xfrm.flip_h = local_attr_val(e, b"flipH").is_some_and(|v| parse_bool(&v));
    xfrm.flip_v = local_attr_val(e, b"flipV").is_some_and(|v| parse_bool(&v));
    xfrm
}

fn read_xfrm_children<R: BufRead>(
    reader: &mut Reader<R>,
    xfrm: &mut Xfrm,
) -> Result<(), quick_xml::Error> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => match local_name(&e) {
                b"off" => {
                    xfrm.x_emu = attr_i64(&e, b"x");
                    xfrm.y_emu = attr_i64(&e, b"y");
                }
                b"ext" => {
                    xfrm.cx_emu = attr_i64(&e, b"cx");
                    xfrm.cy_emu = attr_i64(&e, b"cy");
                }
                _ => {}
            },
            Event::End(e) if end_is(&e, b"xfrm") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_ln<R: BufRead>(
    reader: &mut Reader<R>,
    start: &BytesStart<'_>,
) -> Result<Option<Stroke>, quick_xml::Error> {
    let width_pt = local_attr_val(start, b"w")
        .and_then(|w| w.parse::<i64>().ok())
        .map_or(1.0, emu_to_pt);

    let mut color = None;
    let mut no_fill = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"solidFill" => color = parse_solid_fill_color(reader)?,
                _ => skip_subtree(reader)?,
            },
            Event::Empty(e) => match local_name(&e) {
                b"noFill" => no_fill = true,
                b"srgbClr" => {
                    if let Some(hex) = local_attr_val(&e, b"val") {
                        color = color_from_srgb(&hex);
                    }
                }
                _ => {}
            },
            Event::End(e) if end_is(&e, b"ln") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if no_fill {
        return Ok(None);
    }
    Ok(Some(Stroke::solid(
        color.unwrap_or_else(default_black),
        width_pt,
    )))
}

fn attr_i64(e: &BytesStart<'_>, name: &[u8]) -> i64 {
    local_attr_val(e, name)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
}

/// Parses the `adj` guide from a `<a:prstGeom><a:avLst>` subtree and stores it
/// in `preset` as a normalised fraction (0.0–0.5 for `roundRect`).
///
/// Consumes through the closing `</a:prstGeom>` tag.
fn parse_avlst_adj<R: BufRead>(
    reader: &mut Reader<R>,
    preset: &mut PresetShape,
) -> Result<(), quick_xml::Error> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) if local_name(&e) == b"gd" => {
                if local_attr_val(&e, b"name").as_deref() == Some("adj") {
                    if let Some(fmla) = local_attr_val(&e, b"fmla") {
                        if let Some(val_str) = fmla.strip_prefix("val ") {
                            if let Ok(adj) = val_str.trim().parse::<f64>() {
                                *preset = PresetShape::RoundedRectangle {
                                    corner_radius: adj / 100_000.0,
                                };
                            }
                        }
                    }
                }
            }
            Event::End(e) if end_is(&e, b"prstGeom") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}
