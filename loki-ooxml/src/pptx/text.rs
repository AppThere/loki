// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parses a `DrawingML` `p:txBody` / `a:txBody` into a [`TextBody`].

use loki_graphics::{TextAlign, TextBody, TextParagraph, TextRun, TextRunProps, VerticalAnchor};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::BufRead;

use super::units::{color_from_srgb, font_size_to_pt, parse_bool};
use super::{end_is, parse_solid_fill_color, skip_subtree};
use crate::xml_util::{local_attr_val, local_name};

/// Parses a text body. Assumes the opening `txBody` start tag was already read;
/// consumes through its end tag.
pub(super) fn parse_txbody<R: BufRead>(
    reader: &mut Reader<R>,
) -> Result<TextBody, quick_xml::Error> {
    let mut paragraphs = Vec::new();
    let mut anchor = VerticalAnchor::Top;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"bodyPr" => {
                    if let Some(a) = local_attr_val(&e, b"anchor") {
                        anchor = anchor_from(&a);
                    }
                    skip_subtree(reader)?;
                }
                b"p" => paragraphs.push(parse_paragraph(reader)?),
                _ => skip_subtree(reader)?,
            },
            Event::Empty(e) => {
                if local_name(&e) == b"bodyPr" {
                    if let Some(a) = local_attr_val(&e, b"anchor") {
                        anchor = anchor_from(&a);
                    }
                }
            }
            Event::End(e) if end_is(&e, b"txBody") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(TextBody { paragraphs, anchor })
}

fn anchor_from(val: &str) -> VerticalAnchor {
    match val {
        "ctr" => VerticalAnchor::Middle,
        "b" => VerticalAnchor::Bottom,
        _ => VerticalAnchor::Top,
    }
}

fn parse_paragraph<R: BufRead>(reader: &mut Reader<R>) -> Result<TextParagraph, quick_xml::Error> {
    let mut runs = Vec::new();
    let mut align = TextAlign::Left;
    let mut level = 0u8;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"pPr" => {
                    apply_ppr(&e, &mut align, &mut level);
                    skip_subtree(reader)?;
                }
                b"r" => runs.push(parse_run(reader)?),
                _ => skip_subtree(reader)?,
            },
            Event::Empty(e) => {
                if local_name(&e) == b"pPr" {
                    apply_ppr(&e, &mut align, &mut level);
                }
            }
            Event::End(e) if end_is(&e, b"p") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(TextParagraph { runs, align, level })
}

fn apply_ppr(e: &quick_xml::events::BytesStart<'_>, align: &mut TextAlign, level: &mut u8) {
    if let Some(a) = local_attr_val(e, b"algn") {
        *align = match a.as_str() {
            "ctr" => TextAlign::Center,
            "r" => TextAlign::Right,
            "just" => TextAlign::Justify,
            _ => TextAlign::Left,
        };
    }
    if let Some(l) = local_attr_val(e, b"lvl") {
        if let Ok(n) = l.parse::<u8>() {
            *level = n;
        }
    }
}

fn parse_run<R: BufRead>(reader: &mut Reader<R>) -> Result<TextRun, quick_xml::Error> {
    let mut text = String::new();
    let mut props = TextRunProps::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"rPr" => {
                    apply_rpr_attrs(&e, &mut props);
                    parse_rpr_children(reader, &mut props)?;
                }
                b"t" => text.push_str(&read_text(reader)?),
                _ => skip_subtree(reader)?,
            },
            Event::Empty(e) => {
                if local_name(&e) == b"rPr" {
                    apply_rpr_attrs(&e, &mut props);
                }
            }
            Event::End(e) if end_is(&e, b"r") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(TextRun { text, props })
}

fn apply_rpr_attrs(e: &quick_xml::events::BytesStart<'_>, props: &mut TextRunProps) {
    if let Some(b) = local_attr_val(e, b"b") {
        props.bold = parse_bool(&b);
    }
    if let Some(i) = local_attr_val(e, b"i") {
        props.italic = parse_bool(&i);
    }
    if let Some(u) = local_attr_val(e, b"u") {
        // `u` is an enum ("none", "sng", "dbl", …); any non-"none" is underline.
        props.underline = u != "none";
    }
    if let Some(sz) = local_attr_val(e, b"sz") {
        if let Ok(n) = sz.parse::<i64>() {
            props.font_size_pt = Some(font_size_to_pt(n));
        }
    }
}

fn parse_rpr_children<R: BufRead>(
    reader: &mut Reader<R>,
    props: &mut TextRunProps,
) -> Result<(), quick_xml::Error> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match local_name(&e) {
                b"solidFill" => props.color = parse_solid_fill_color(reader)?,
                _ => skip_subtree(reader)?,
            },
            Event::Empty(e) => {
                if local_name(&e) == b"latin" {
                    props.font_family = local_attr_val(&e, b"typeface");
                } else if local_name(&e) == b"srgbClr" {
                    // A bare srgbClr directly under rPr is unusual but tolerated.
                    if let Some(hex) = local_attr_val(&e, b"val") {
                        props.color = color_from_srgb(&hex);
                    }
                }
            }
            Event::End(e) if end_is(&e, b"rPr") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn read_text<R: BufRead>(reader: &mut Reader<R>) -> Result<String, quick_xml::Error> {
    let mut s = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Text(t) => s.push_str(&t.unescape()?),
            Event::End(e) if end_is(&e, b"t") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(s)
}
