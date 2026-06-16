// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parses a slide shape tree (`p:spTree`) into [`ParsedShape`]s.

use loki_graphics::{
    Geometry, GeometryShape, ImageFormat, ImageRef, ImageShape, ImageSource, RectF, Shape,
    ShapeKind, ShapeTransform,
};
use loki_presentation_model::PlaceholderKind;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::io::BufRead;

use super::sppr::{ShapeProps, Xfrm, parse_sppr};
use super::text::parse_txbody;
use super::units::emu_to_pt;
use super::{ParsedShape, end_is, skip_subtree};
use crate::error::OoxmlWarning;
use crate::xml_util::{local_attr_val, local_name};

/// Parses a `p:spTree`. Assumes the opening tag was already read; consumes
/// through its end tag. Unsupported shape kinds are skipped and recorded.
pub(super) fn parse_sptree<R: BufRead>(
    reader: &mut Reader<R>,
    warnings: &mut Vec<OoxmlWarning>,
) -> Result<Vec<ParsedShape>, quick_xml::Error> {
    let mut shapes = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"sp" => {
                    if let Some(parsed) = parse_sp(reader)? {
                        shapes.push(parsed);
                    }
                }
                b"pic" => {
                    if let Some(parsed) = parse_pic(reader)? {
                        shapes.push(parsed);
                    }
                }
                // The spTree's own group props, plus unsupported shape kinds.
                b"nvGrpSpPr" | b"grpSpPr" => skip_subtree(reader)?,
                other => {
                    warnings.push(OoxmlWarning::Unsupported {
                        feature: format!("p:{}", String::from_utf8_lossy(other)),
                    });
                    skip_subtree(reader)?;
                }
            },
            Event::End(e) if end_is(&e, b"spTree") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(shapes)
}

/// Parsed identity/non-visual properties of a shape.
#[derive(Default)]
struct NvProps {
    id: Option<String>,
    name: Option<String>,
    placeholder: Option<PlaceholderKind>,
}

fn parse_sp<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<ParsedShape>, quick_xml::Error> {
    let mut nv = NvProps::default();
    let mut props = ShapeProps::default();
    let mut text = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"nvSpPr" => nv = parse_nvsppr(reader)?,
                b"spPr" => props = parse_sppr(reader)?,
                b"txBody" => text = Some(parse_txbody(reader)?),
                _ => skip_subtree(reader)?,
            },
            Event::End(e) if end_is(&e, b"sp") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let frame = frame_from(props.xfrm.as_ref());
    let transform = transform_from(props.xfrm.as_ref(), frame);
    let geometry_shape = GeometryShape {
        geometry: Geometry::Preset(props.preset),
        fill: props.fill,
        stroke: props.stroke,
        text,
    };
    let shape = Shape {
        id: nv.id.unwrap_or_default().into(),
        name: nv.name,
        transform,
        kind: ShapeKind::Geometry(geometry_shape),
    };
    Ok(Some(ParsedShape {
        shape,
        placeholder: nv.placeholder,
    }))
}

fn parse_pic<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<ParsedShape>, quick_xml::Error> {
    let mut nv = NvProps::default();
    let mut props = ShapeProps::default();
    let mut embed_rid = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"nvPicPr" => nv = parse_nvsppr(reader)?,
                b"spPr" => props = parse_sppr(reader)?,
                b"blipFill" => embed_rid = parse_blip_fill(reader)?,
                _ => skip_subtree(reader)?,
            },
            Event::End(e) if end_is(&e, b"pic") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let Some(rid) = embed_rid else {
        return Ok(None); // a picture with no image reference is not useful
    };
    let frame = frame_from(props.xfrm.as_ref());
    let transform = transform_from(props.xfrm.as_ref(), frame);
    // The relationship id is resolved to embedded bytes by the orchestrator;
    // store it as an external reference until then.
    let shape = Shape {
        id: nv.id.unwrap_or_default().into(),
        name: nv.name,
        transform,
        kind: ShapeKind::Image(ImageShape {
            image: ImageRef {
                format: ImageFormat::Other(rid.clone()),
                source: ImageSource::External(rid),
            },
        }),
    };
    Ok(Some(ParsedShape {
        shape,
        placeholder: nv.placeholder,
    }))
}

/// Parses `p:nvSpPr` / `p:nvPicPr`: shape id, name, and placeholder role.
fn parse_nvsppr<R: BufRead>(reader: &mut Reader<R>) -> Result<NvProps, quick_xml::Error> {
    let mut nv = NvProps::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => match local_name(&e) {
                b"cNvPr" => {
                    nv.id = local_attr_val(&e, b"id");
                    nv.name = local_attr_val(&e, b"name");
                }
                b"ph" => nv.placeholder = Some(placeholder_from(&e)),
                _ => {}
            },
            Event::End(e) if end_is(&e, b"nvSpPr") || end_is(&e, b"nvPicPr") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(nv)
}

fn parse_blip_fill<R: BufRead>(reader: &mut Reader<R>) -> Result<Option<String>, quick_xml::Error> {
    let mut rid = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                if local_name(&e) == b"blip" {
                    rid = local_attr_val(&e, b"embed");
                }
            }
            Event::End(e) if end_is(&e, b"blipFill") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(rid)
}

fn placeholder_from(e: &BytesStart<'_>) -> PlaceholderKind {
    match local_attr_val(e, b"type").as_deref() {
        Some("title") => PlaceholderKind::Title,
        Some("ctrTitle") => PlaceholderKind::CenteredTitle,
        Some("subTitle") => PlaceholderKind::Subtitle,
        // No `type` means a body/object placeholder by default.
        Some("body") | None => PlaceholderKind::Body,
        _ => PlaceholderKind::Other,
    }
}

fn frame_from(xfrm: Option<&Xfrm>) -> RectF {
    xfrm.map_or(RectF::default(), |x| {
        RectF::new(
            emu_to_pt(x.x_emu),
            emu_to_pt(x.y_emu),
            emu_to_pt(x.cx_emu),
            emu_to_pt(x.cy_emu),
        )
    })
}

fn transform_from(xfrm: Option<&Xfrm>, frame: RectF) -> ShapeTransform {
    let mut t = ShapeTransform::new(frame);
    if let Some(x) = xfrm {
        t.rotation_deg = super::units::rot_to_deg(x.rot);
        t.flip_h = x.flip_h;
        t.flip_v = x.flip_v;
    }
    t
}
