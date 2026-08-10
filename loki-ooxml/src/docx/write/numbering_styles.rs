// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialises a catalog [`ListStyle`] as a full multi-level `w:abstractNum`
//! (usage audit §10 tier 2 — previously every level was flattened to one
//! hardcoded `w:lvl`, which destroyed nested lists on export).
//!
//! ECMA-376 §17.9. The model's `%N` label format is OOXML's `w:lvlText`
//! syntax, so formats pass through verbatim.

use quick_xml::Writer;

use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};

use crate::docx::write::xml::{
    pts_to_twips, write_empty, write_end, write_start, write_text_elem, wval,
};

/// Maps a [`NumberingScheme`] to the OOXML `w:numFmt` value.
fn num_fmt(scheme: NumberingScheme) -> &'static str {
    match scheme {
        NumberingScheme::LowerAlpha => "lowerLetter",
        NumberingScheme::UpperAlpha => "upperLetter",
        NumberingScheme::LowerRoman => "lowerRoman",
        NumberingScheme::UpperRoman => "upperRoman",
        NumberingScheme::Ordinal => "ordinal",
        NumberingScheme::None => "none",
        // Decimal, plus any #[non_exhaustive] future scheme this writer has
        // not learned yet, degrades to decimal rather than invalid XML.
        _ => "decimal",
    }
}

/// Maps a [`LabelAlignment`] to the OOXML `w:lvlJc` value.
fn lvl_jc(alignment: LabelAlignment) -> &'static str {
    match alignment {
        LabelAlignment::Right => "right",
        LabelAlignment::Center => "center",
        _ => "left",
    }
}

/// Writes one `w:lvl` from a [`ListLevel`].
fn write_level<W: std::io::Write>(w: &mut Writer<W>, lvl: &ListLevel) {
    let level_attr = lvl.level.to_string();
    let _ = write_start(w, "w:lvl", &[("w:ilvl", &level_attr)]);

    // CT_Lvl child order: start, numFmt, …, lvlText, lvlJc, pPr, rPr.
    let (fmt, start, text, bullet_font) = match &lvl.kind {
        ListLevelKind::Bullet { char, font } => {
            let text = match char {
                BulletChar::Char(c) => c.to_string(),
                // A picture bullet's image cannot ride this simple path (it
                // needs w:numPicBullet + a media part); its glyph fallback —
                // like any #[non_exhaustive] future variant — is the plain
                // bullet, matching the importer's PUA normalisation.
                _ => "\u{2022}".to_string(),
            };
            ("bullet", 1, text, font.as_deref())
        }
        ListLevelKind::Numbered {
            scheme,
            start_value,
            format,
            ..
        } => (num_fmt(*scheme), *start_value, format.clone(), None),
        // #[non_exhaustive]: an unknown kind degrades to a plain bullet.
        _ => ("bullet", 1, "\u{2022}".to_string(), None),
    };

    let start_s = start.to_string();
    let _ = write_empty(w, "w:start", &wval(&start_s));
    let _ = write_empty(w, "w:numFmt", &wval(fmt));
    let _ = write_text_elem(w, "w:lvlText", &wval(&text), "");
    let _ = write_empty(w, "w:lvlJc", &wval(lvl_jc(lvl.label_alignment)));

    let left = pts_to_twips(lvl.indent_start.value()).to_string();
    let hanging = pts_to_twips(lvl.hanging_indent.value()).to_string();
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_empty(w, "w:ind", &[("w:left", &left), ("w:hanging", &hanging)]);
    let _ = write_end(w, "w:pPr");

    if let Some(font) = bullet_font {
        let _ = write_start(w, "w:rPr", &[]);
        let _ = write_empty(
            w,
            "w:rFonts",
            &[("w:ascii", font), ("w:hAnsi", font), ("w:cs", font)],
        );
        let _ = write_end(w, "w:rPr");
    }

    let _ = write_end(w, "w:lvl");
}

/// Writes the body of a `w:abstractNum` (its `w:multiLevelType` + every
/// defined `w:lvl`) from a catalog [`ListStyle`]. The caller opens and closes
/// the `w:abstractNum` element itself.
pub(super) fn write_styled_abstract_body<W: std::io::Write>(w: &mut Writer<W>, style: &ListStyle) {
    let kind = if style.levels.len() > 1 {
        "multilevel"
    } else {
        "singleLevel"
    };
    let _ = write_empty(w, "w:multiLevelType", &wval(kind));
    for lvl in &style.levels {
        write_level(w, lvl);
    }
}
