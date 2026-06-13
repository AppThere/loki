// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialises a slide part (`ppt/slides/slideN.xml`) from the model.
//!
//! Inverse of [`super::shapes`]/[`super::sppr`]/[`super::text`]. Exports
//! geometry shapes (preset geometry, solid fill, line stroke, text, placeholder
//! role). Groups, images, gradients, and custom-path geometry are not yet
//! exported (custom paths fall back to a rectangle); these are tracked
//! follow-ups, matching the importer's coverage.

use std::collections::HashMap;
use std::fmt::Write as _;

use loki_graphics::{Fill, Geometry, GeometryShape, Shape, ShapeKind, ShapeTransform, Stroke};
use loki_graphics::{TextAlign, TextBody, TextParagraph, TextRun, VerticalAnchor};
use loki_presentation_model::{PlaceholderKind, Slide};

use super::escape_xml;
use super::units::{deg_to_rot, prst_from_preset, pt_to_emu, pt_to_font_size, srgb_from_color};

const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

/// Builds `ppt/slides/slideN.xml` for `slide`.
pub(super) fn slide_xml(slide: &Slide) -> String {
    let placeholders: HashMap<&str, PlaceholderKind> = slide
        .placeholders
        .iter()
        .map(|p| (p.shape_id.as_str(), p.kind))
        .collect();

    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(
        s,
        "<p:sld xmlns:p=\"{NS_P}\" xmlns:r=\"{NS_R}\" xmlns:a=\"{NS_A}\"><p:cSld><p:spTree>"
    );
    // The required, otherwise-empty group-shape header.
    s.push_str(
        "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>",
    );

    let mut next_id = 2u32;
    for shape in &slide.drawing.shapes {
        if let ShapeKind::Geometry(g) = &shape.kind {
            let ph = placeholders.get(shape.id.as_str()).copied();
            write_sp(&mut s, shape, g, ph, &mut next_id);
        }
    }

    s.push_str("</p:spTree></p:cSld></p:sld>");
    s
}

fn write_sp(
    s: &mut String,
    shape: &Shape,
    g: &GeometryShape,
    placeholder: Option<PlaceholderKind>,
    next_id: &mut u32,
) {
    let id = shape
        .id
        .as_str()
        .parse::<u32>()
        .ok()
        .filter(|&n| n != 0)
        .unwrap_or_else(|| {
            let v = *next_id;
            *next_id += 1;
            v
        });
    let name = shape.name.as_deref().unwrap_or("");

    let _ = write!(
        s,
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{}\"/><p:cNvSpPr/>",
        escape_xml(name)
    );
    match placeholder {
        Some(kind) => match ph_type(kind) {
            Some(t) => {
                let _ = write!(s, "<p:nvPr><p:ph type=\"{t}\"/></p:nvPr>");
            }
            None => s.push_str("<p:nvPr><p:ph/></p:nvPr>"),
        },
        None => s.push_str("<p:nvPr/>"),
    }
    s.push_str("</p:nvSpPr><p:spPr>");

    write_xfrm(s, &shape.transform);
    let prst = match &g.geometry {
        Geometry::Preset(p) => prst_from_preset(*p),
        Geometry::Custom(_) => "rect",
    };
    let _ = write!(s, "<a:prstGeom prst=\"{prst}\"><a:avLst/></a:prstGeom>");
    write_fill(s, &g.fill);
    write_ln(s, g.stroke.as_ref());
    s.push_str("</p:spPr>");

    if let Some(tb) = &g.text {
        write_txbody(s, tb);
    }
    s.push_str("</p:sp>");
}

fn ph_type(kind: PlaceholderKind) -> Option<&'static str> {
    match kind {
        PlaceholderKind::Title => Some("title"),
        PlaceholderKind::CenteredTitle => Some("ctrTitle"),
        PlaceholderKind::Subtitle => Some("subTitle"),
        PlaceholderKind::Body | PlaceholderKind::Notes => Some("body"),
        PlaceholderKind::Other => None,
    }
}

fn write_xfrm(s: &mut String, t: &ShapeTransform) {
    let rot = deg_to_rot(t.rotation_deg);
    s.push_str("<a:xfrm");
    if rot != 0 {
        let _ = write!(s, " rot=\"{rot}\"");
    }
    if t.flip_h {
        s.push_str(" flipH=\"1\"");
    }
    if t.flip_v {
        s.push_str(" flipV=\"1\"");
    }
    s.push('>');
    let f = &t.frame;
    let _ = write!(
        s,
        "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
        pt_to_emu(f.x),
        pt_to_emu(f.y),
        pt_to_emu(f.width),
        pt_to_emu(f.height),
    );
    s.push_str("</a:xfrm>");
}

fn write_fill(s: &mut String, fill: &Fill) {
    if let Fill::Solid(color) = fill
        && let Some(hex) = srgb_from_color(color)
    {
        let _ = write!(s, "<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>");
    }
    // Fill::None and gradients emit nothing (gradient export is a follow-up).
}

fn write_ln(s: &mut String, stroke: Option<&Stroke>) {
    let Some(st) = stroke else { return };
    let _ = write!(s, "<a:ln w=\"{}\">", pt_to_emu(st.width_pt));
    if let Some(hex) = srgb_from_color(&st.color) {
        let _ = write!(s, "<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>");
    }
    s.push_str("</a:ln>");
}

fn write_txbody(s: &mut String, tb: &TextBody) {
    s.push_str("<p:txBody><a:bodyPr");
    match tb.anchor {
        VerticalAnchor::Top => {}
        VerticalAnchor::Middle => s.push_str(" anchor=\"ctr\""),
        VerticalAnchor::Bottom => s.push_str(" anchor=\"b\""),
    }
    s.push_str("/><a:lstStyle/>");
    for para in &tb.paragraphs {
        write_paragraph(s, para);
    }
    s.push_str("</p:txBody>");
}

fn write_paragraph(s: &mut String, para: &TextParagraph) {
    s.push_str("<a:p>");
    let algn = match para.align {
        TextAlign::Left => None,
        TextAlign::Center => Some("ctr"),
        TextAlign::Right => Some("r"),
        TextAlign::Justify => Some("just"),
    };
    if algn.is_some() || para.level > 0 {
        s.push_str("<a:pPr");
        if para.level > 0 {
            let _ = write!(s, " lvl=\"{}\"", para.level);
        }
        if let Some(a) = algn {
            let _ = write!(s, " algn=\"{a}\"");
        }
        s.push_str("/>");
    }
    for run in &para.runs {
        write_run(s, run);
    }
    s.push_str("</a:p>");
}

fn write_run(s: &mut String, run: &TextRun) {
    let p = &run.props;
    s.push_str("<a:r><a:rPr");
    if p.bold {
        s.push_str(" b=\"1\"");
    }
    if p.italic {
        s.push_str(" i=\"1\"");
    }
    if p.underline {
        s.push_str(" u=\"sng\"");
    }
    if let Some(sz) = p.font_size_pt {
        let _ = write!(s, " sz=\"{}\"", pt_to_font_size(sz));
    }

    let color_hex = p.color.as_ref().and_then(srgb_from_color);
    let has_children = color_hex.is_some() || p.font_family.is_some();
    if has_children {
        s.push('>');
        if let Some(hex) = color_hex {
            let _ = write!(s, "<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>");
        }
        if let Some(font) = &p.font_family {
            let _ = write!(s, "<a:latin typeface=\"{}\"/>", escape_xml(font));
        }
        s.push_str("</a:rPr>");
    } else {
        s.push_str("/>");
    }

    let _ = write!(s, "<a:t>{}</a:t></a:r>", escape_xml(&run.text));
}
