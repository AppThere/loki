// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialises [`ParaProps`] to a `style:paragraph-properties` element, mirroring
//! the import mapper (`odt::mapper::props::paragraph`) so every property it reads
//! back round-trips (alignment, indents, spacing, line height, flow control,
//! borders, padding, tab stops, bidi, background).

use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::props::para_props::{
    LineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use super::xml::{attr, pt};

/// Emits a complete `<style:paragraph-properties>` element (with any tab-stop
/// children) from `pp`, or an empty string when `pp` carries no formatting.
#[must_use]
pub(super) fn emit_paragraph_properties(pp: &ParaProps) -> String {
    let a = paragraph_properties_attrs(pp);
    let children = tab_stops_xml(pp.tab_stops.as_deref().unwrap_or(&[]));
    if a.is_empty() && children.is_empty() {
        return String::new();
    }
    if children.is_empty() {
        format!("<style:paragraph-properties{a}/>")
    } else {
        format!("<style:paragraph-properties{a}>{children}</style:paragraph-properties>")
    }
}

fn paragraph_properties_attrs(pp: &ParaProps) -> String {
    let mut s = String::new();
    if let Some(a) = pp.alignment {
        let v = match a {
            ParagraphAlignment::Right => "end",
            ParagraphAlignment::Center => "center",
            ParagraphAlignment::Justify | ParagraphAlignment::Distribute => "justify",
            _ => "start",
        };
        attr(&mut s, "fo:text-align", v);
    }
    if let Some(Spacing::Exact(p)) = pp.space_before {
        attr(&mut s, "fo:margin-top", &pt(p));
    }
    if let Some(Spacing::Exact(p)) = pp.space_after {
        attr(&mut s, "fo:margin-bottom", &pt(p));
    }
    if let Some(p) = pp.indent_start {
        attr(&mut s, "fo:margin-left", &pt(p));
    }
    if let Some(p) = pp.indent_end {
        attr(&mut s, "fo:margin-right", &pt(p));
    }
    // ODF expresses a hanging indent as a negative fo:text-indent; first-line
    // indent as a positive one (mutually exclusive — hanging wins).
    if let Some(p) = pp.indent_hanging {
        attr(&mut s, "fo:text-indent", &format!("-{}", pt(p)));
    } else if let Some(p) = pp.indent_first_line {
        attr(&mut s, "fo:text-indent", &pt(p));
    }
    match pp.line_height {
        Some(LineHeight::Multiple(m)) => {
            attr(&mut s, "fo:line-height", &format!("{:.0}%", m * 100.0));
        }
        Some(LineHeight::Exact(p)) => attr(&mut s, "fo:line-height", &pt(p)),
        Some(LineHeight::AtLeast(p)) => attr(&mut s, "style:line-height-at-least", &pt(p)),
        _ => {}
    }
    if pp.keep_together == Some(true) {
        attr(&mut s, "fo:keep-together", "always");
    }
    if pp.keep_with_next == Some(true) {
        attr(&mut s, "fo:keep-with-next", "always");
    }
    if let Some(n) = pp.widow_control {
        attr(&mut s, "fo:widows", &n.to_string());
    }
    if let Some(n) = pp.orphan_control {
        attr(&mut s, "fo:orphans", &n.to_string());
    }
    if pp.page_break_before == Some(true) {
        attr(&mut s, "fo:break-before", "page");
    }
    if pp.page_break_after == Some(true) {
        attr(&mut s, "fo:break-after", "page");
    }
    border_attr(&mut s, "fo:border-top", pp.border_top.as_ref());
    border_attr(&mut s, "fo:border-bottom", pp.border_bottom.as_ref());
    border_attr(&mut s, "fo:border-left", pp.border_left.as_ref());
    border_attr(&mut s, "fo:border-right", pp.border_right.as_ref());
    emit_padding(&mut s, pp);
    if pp.bidi == Some(true) {
        attr(&mut s, "style:writing-mode", "rl-tb");
    }
    if let Some(hex) = pp.background_color.as_ref().and_then(DocumentColor::to_hex) {
        attr(&mut s, "fo:background-color", &hex);
    }
    s
}

fn padding_attr(s: &mut String, name: &str, p: Option<Points>) {
    if let Some(p) = p {
        attr(s, name, &pt(p));
    }
}

/// Emits paragraph padding. Uses the `fo:padding` shorthand when all four sides
/// are equal (the common case, which the importer reads back); otherwise emits
/// each side (valid ODF, preserved for other apps).
fn emit_padding(s: &mut String, pp: &ParaProps) {
    let top = pp.padding_top;
    let (bottom, left, right) = (pp.padding_bottom, pp.padding_left, pp.padding_right);
    if let (Some(p), true) = (top, top == bottom && bottom == left && left == right) {
        attr(s, "fo:padding", &pt(p));
    } else {
        padding_attr(s, "fo:padding-top", top);
        padding_attr(s, "fo:padding-bottom", bottom);
        padding_attr(s, "fo:padding-left", left);
        padding_attr(s, "fo:padding-right", right);
    }
}

/// Appends a border attribute as the ODF `"width style color"` shorthand.
pub(super) fn border_attr(s: &mut String, name: &str, border: Option<&Border>) {
    if let Some(b) = border {
        let style = match b.style {
            BorderStyle::None => {
                attr(s, name, "none");
                return;
            }
            BorderStyle::Dashed => "dashed",
            BorderStyle::Dotted => "dotted",
            BorderStyle::Double => "double",
            BorderStyle::Groove => "groove",
            BorderStyle::Ridge => "ridge",
            BorderStyle::Inset => "inset",
            BorderStyle::Outset => "outset",
            BorderStyle::Wave => "wave",
            _ => "solid",
        };
        let mut val = format!("{} {style}", pt(b.width));
        if let Some(hex) = b.color.as_ref().and_then(DocumentColor::to_hex) {
            val.push(' ');
            val.push_str(&hex);
        }
        attr(s, name, &val);
    }
}

/// Renders `<style:tab-stops>` from a paragraph's tab stops (empty when none).
fn tab_stops_xml(stops: &[TabStop]) -> String {
    if stops.is_empty() {
        return String::new();
    }
    let mut s = String::from("<style:tab-stops>");
    for ts in stops {
        s.push_str("<style:tab-stop");
        attr(&mut s, "style:position", &pt(ts.position));
        let kind = match ts.alignment {
            TabAlignment::Right => Some("right"),
            TabAlignment::Center => Some("center"),
            TabAlignment::Decimal => Some("char"),
            _ => None,
        };
        if let Some(kind) = kind {
            attr(&mut s, "style:type", kind);
        }
        let leader = match ts.leader {
            TabLeader::Dot | TabLeader::MiddleDot => Some("dotted"),
            TabLeader::Dash => Some("dash"),
            TabLeader::Underscore => Some("solid"),
            TabLeader::Heavy => Some("bold"),
            _ => None,
        };
        if let Some(leader) = leader {
            attr(&mut s, "style:leader-style", leader);
        }
        s.push_str("/>");
    }
    s.push_str("</style:tab-stops>");
    s
}
