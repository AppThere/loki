// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialises [`CharProps`] / [`ParaProps`] to ODF `style:*-properties`
//! attribute strings. These mirror the import mappers
//! (`odt::mapper::props`) so styles round-trip.

use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};
use loki_doc_model::style::props::para_props::{
    LineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_primitives::color::DocumentColor;

use super::xml::{attr, pt};

/// Builds the attribute list for a `<style:text-properties>` element from `cp`.
/// Returns an empty string when no property is set.
#[must_use]
pub(super) fn text_properties_attrs(cp: &CharProps) -> String {
    let mut s = String::new();
    if let Some(f) = &cp.font_name {
        attr(&mut s, "style:font-name", f);
        attr(&mut s, "fo:font-family", f);
    }
    if let Some(sz) = cp.font_size {
        attr(&mut s, "fo:font-size", &pt(sz));
    }
    match cp.bold {
        Some(true) => attr(&mut s, "fo:font-weight", "bold"),
        Some(false) => attr(&mut s, "fo:font-weight", "normal"),
        None => {}
    }
    match cp.italic {
        Some(true) => attr(&mut s, "fo:font-style", "italic"),
        Some(false) => attr(&mut s, "fo:font-style", "normal"),
        None => {}
    }
    if let Some(u) = cp.underline {
        let v = match u {
            UnderlineStyle::Double => "double",
            UnderlineStyle::Dotted => "dotted",
            UnderlineStyle::Dash => "dash",
            UnderlineStyle::Wave => "wave",
            UnderlineStyle::Thick => "bold",
            _ => "solid",
        };
        attr(&mut s, "style:text-underline-style", v);
        attr(&mut s, "style:text-underline-width", "auto");
        attr(&mut s, "style:text-underline-color", "font-color");
    }
    if let Some(st) = cp.strikethrough {
        let v = match st {
            StrikethroughStyle::Double => "double",
            _ => "solid",
        };
        attr(&mut s, "style:text-line-through-style", v);
    }
    if cp.small_caps == Some(true) {
        attr(&mut s, "fo:font-variant", "small-caps");
    }
    if cp.all_caps == Some(true) {
        attr(&mut s, "fo:text-transform", "uppercase");
    }
    if let Some(va) = cp.vertical_align {
        let v = match va {
            VerticalAlign::Superscript => "super 58%",
            VerticalAlign::Subscript => "sub 58%",
            _ => "0%",
        };
        attr(&mut s, "style:text-position", v);
    }
    if let Some(hex) = cp.color.as_ref().and_then(DocumentColor::to_hex) {
        attr(&mut s, "fo:color", &hex);
    }
    if let Some(hex) = cp.background_color.as_ref().and_then(DocumentColor::to_hex) {
        attr(&mut s, "fo:background-color", &hex);
    }
    if let Some(ls) = cp.letter_spacing {
        attr(&mut s, "fo:letter-spacing", &pt(ls));
    }
    if let Some(lang) = &cp.language {
        match lang.as_str().split_once('-') {
            Some((l, c)) => {
                attr(&mut s, "fo:language", l);
                attr(&mut s, "fo:country", c);
            }
            None => attr(&mut s, "fo:language", lang.as_str()),
        }
    }
    s
}

/// Builds the attribute list for a `<style:paragraph-properties>` element.
/// Returns an empty string when no property is set.
#[must_use]
pub(super) fn paragraph_properties_attrs(pp: &ParaProps) -> String {
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
    if pp.page_break_before == Some(true) {
        attr(&mut s, "fo:break-before", "page");
    }
    if pp.page_break_after == Some(true) {
        attr(&mut s, "fo:break-after", "page");
    }
    if let Some(hex) = pp.background_color.as_ref().and_then(DocumentColor::to_hex) {
        attr(&mut s, "fo:background-color", &hex);
    }
    s
}
