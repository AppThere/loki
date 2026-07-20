// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialises [`CharProps`] to a `style:text-properties` element, mirroring the
//! import mapper (`odt::mapper::props::character`) so every property it reads
//! back round-trips. Paragraph properties live in [`super::para_props`].

use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};
use loki_primitives::color::DocumentColor;

use super::xml::{attr, pt};

/// Emits a complete `<style:text-properties .../>` element from `cp`, or an
/// empty string when `cp` carries no formatting.
#[must_use]
pub(super) fn emit_text_properties(cp: &CharProps) -> String {
    let a = text_properties_attrs(cp);
    if a.is_empty() {
        String::new()
    } else {
        format!("<style:text-properties{a}/>")
    }
}

fn text_properties_attrs(cp: &CharProps) -> String {
    let mut s = String::new();
    if let Some(f) = &cp.font_name {
        attr(&mut s, "style:font-name", f);
        attr(&mut s, "fo:font-family", f);
    }
    if let Some(f) = &cp.font_name_complex {
        attr(&mut s, "style:font-name-complex", f);
    }
    if let Some(f) = &cp.font_name_east_asian {
        attr(&mut s, "style:font-name-asian", f);
    }
    if let Some(sz) = cp.font_size {
        attr(&mut s, "fo:font-size", &pt(sz));
    }
    if let Some(sz) = cp.font_size_complex {
        attr(&mut s, "style:font-size-complex", &pt(sz));
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
        attr(&mut s, "style:text-underline-style", underline_style(u));
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
    if cp.outline == Some(true) {
        attr(&mut s, "style:text-outline", "true");
    }
    if cp.shadow == Some(true) {
        attr(&mut s, "fo:text-shadow", "1pt 1pt");
    }
    // Emboss / imprint → the single ODF style:font-relief (embossed wins if both
    // are set, which the model never produces from a real import). ODF 1.3 §20.191.
    if cp.emboss == Some(true) {
        attr(&mut s, "style:font-relief", "embossed");
    } else if cp.imprint == Some(true) {
        attr(&mut s, "style:font-relief", "engraved");
    }
    // Character border → fo:border shorthand on text-properties, with fo:padding
    // carrying the border↔glyph inset (Border::spacing).
    if let Some(b) = &cp.character_border {
        super::para_props::border_attr(&mut s, "fo:border", Some(b));
        if let Some(pad) = b.spacing {
            attr(&mut s, "fo:padding", &pt(pad));
        }
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
    if let Some(ws) = cp.word_spacing {
        attr(&mut s, "fo:word-spacing", &pt(ws));
    }
    if let Some(k) = cp.kerning {
        attr(
            &mut s,
            "style:letter-kerning",
            if k { "true" } else { "false" },
        );
    }
    if let Some(scale) = cp.scale {
        attr(&mut s, "style:text-scale", &format!("{scale:.0}%"));
    }
    lang_attrs(&mut s, cp.language.as_ref(), "fo:language", "fo:country");
    lang_attrs(
        &mut s,
        cp.language_complex.as_ref(),
        "style:language-complex",
        "style:country-complex",
    );
    lang_attrs(
        &mut s,
        cp.language_east_asian.as_ref(),
        "style:language-asian",
        "style:country-asian",
    );
    s
}

/// Appends `lang_attr` / `country_attr` for an optional BCP 47 tag (`xx-YY`).
fn lang_attrs(
    s: &mut String,
    tag: Option<&loki_doc_model::meta::LanguageTag>,
    lang_attr: &str,
    country_attr: &str,
) {
    if let Some(tag) = tag {
        match tag.as_str().split_once('-') {
            Some((l, c)) => {
                attr(s, lang_attr, l);
                attr(s, country_attr, c);
            }
            None => attr(s, lang_attr, tag.as_str()),
        }
    }
}

fn underline_style(u: UnderlineStyle) -> &'static str {
    match u {
        UnderlineStyle::Double => "double",
        UnderlineStyle::Dotted => "dotted",
        UnderlineStyle::Dash => "dash",
        UnderlineStyle::Wave => "wave",
        UnderlineStyle::Thick => "bold",
        _ => "solid",
    }
}
