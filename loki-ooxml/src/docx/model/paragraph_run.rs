// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate model for OOXML run elements (split from `paragraph.rs` for
//! the 300-line ceiling): `w:r` and its run properties (`w:rPr`), fonts,
//! hyperlinks, and inline drawings. Mirrors ECMA-376 §17.3.2. Re-exported
//! from `paragraph.rs` so existing paths are unchanged.

use crate::docx::model::revision::DocxMarkRevision;

/// Intermediate model for `w:r` (ECMA-376 §17.3.2.25).
#[derive(Debug, Clone, Default)]
pub struct DocxRun {
    /// Run properties from `w:rPr`.
    pub rpr: Option<DocxRPr>,
    /// Content children: text, breaks, special elements.
    pub children: Vec<DocxRunChild>,
}

/// A child element of `w:r`.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum DocxRunChild {
    /// `w:t` — text content. `preserve` is true when `xml:space="preserve"`.
    Text { text: String, preserve: bool },
    /// `w:br @w:type` — break (line break, page break, column break).
    Break { break_type: Option<String> },
    /// `w:fldChar @w:fldCharType` — field character.
    FldChar { fld_char_type: String },
    /// `w:instrText` — field instruction text.
    InstrText { text: String },
    /// `w:footnoteReference @w:id`.
    FootnoteRef { id: i32 },
    /// `w:endnoteReference @w:id`.
    EndnoteRef { id: i32 },
    /// `w:drawing` — embedded drawing/image.
    Drawing(DocxDrawing),
    /// `w:tab` — explicit tab character.
    Tab,
}

/// Intermediate model for `w:rPr` (ECMA-376 §17.3.2.28).
#[derive(Debug, Clone, Default)]
pub struct DocxRPr {
    /// `w:rStyle @w:val` — character style id.
    pub style_id: Option<String>,
    /// `w:ins` / `w:del` on a paragraph mark's rPr (the tracked ¶ itself).
    pub mark_rev: Option<DocxMarkRevision>,
    /// `w:b` toggle.
    pub bold: Option<bool>,
    /// `w:i` toggle.
    pub italic: Option<bool>,
    /// `w:u @w:val` — underline style.
    pub underline: Option<String>,
    /// `w:strike` toggle.
    pub strike: Option<bool>,
    /// `w:dstrike` toggle.
    pub dstrike: Option<bool>,
    /// `w:smallCaps` toggle.
    pub small_caps: Option<bool>,
    /// `w:caps` toggle.
    pub all_caps: Option<bool>,
    /// `w:shadow` toggle.
    pub shadow: Option<bool>,
    /// `w:emboss` toggle — embossed (raised) text.
    pub emboss: Option<bool>,
    /// `w:imprint` toggle — imprinted (engraved) text.
    pub imprint: Option<bool>,
    /// `w:color @w:val` — foreground color hex string.
    pub color: Option<String>,
    /// `w:highlight @w:val` — named highlight color.
    pub highlight: Option<String>,
    /// `w:position @w:val` — baseline shift in half-points (positive raises).
    pub position: Option<i32>,
    /// `w:sz @w:val` — font size in half-points.
    pub sz: Option<i32>,
    /// `w:szCs @w:val` — complex-script font size in half-points.
    pub sz_cs: Option<i32>,
    /// `w:rFonts` — font names.
    pub fonts: Option<DocxRFonts>,
    /// `w:kern @w:val` — kerning threshold in half-points.
    pub kern: Option<i32>,
    /// `w:spacing @w:val` — character spacing in twips.
    pub spacing: Option<i32>,
    /// `w:w @w:val` — horizontal scale percentage.
    pub scale: Option<i32>,
    /// `w:lang @w:val` — primary language tag.
    pub lang: Option<String>,
    /// `w:lang @w:bidi` — complex-script language tag.
    pub lang_complex: Option<String>,
    /// `w:lang @w:eastAsia` — East Asian language tag.
    pub lang_east_asian: Option<String>,
    /// `w:vertAlign @w:val` — vertical alignment (`"superscript"`, `"subscript"`).
    pub vert_align: Option<String>,
    /// Run shading fill color from `w:shd @w:fill` (hex, no `#`).
    pub shd_fill: Option<String>,
    /// Run shading pattern from `w:shd @w:val` (e.g. `clear`, `pct25`).
    pub shd_val: Option<String>,
    /// Run shading pattern foreground from `w:shd @w:color` (hex).
    pub shd_color: Option<String>,
    /// `w:outline` toggle — hollow/outline text effect.
    pub outline: Option<bool>,
    /// `w:bdr` — a border box drawn around the run's text (ECMA-376 §17.3.2.4).
    pub bdr: Option<super::DocxBorderEdge>,
}

/// `w:rFonts` font name attributes (ECMA-376 §17.3.2.26).
#[derive(Debug, Clone, Default)]
pub struct DocxRFonts {
    /// `@w:ascii` — ASCII font.
    pub ascii: Option<String>,
    /// `@w:cs` — complex-script font.
    pub cs: Option<String>,
    /// `@w:eastAsia` — East Asian font.
    pub east_asia: Option<String>,
    /// `@w:hAnsi` — high-ANSI font.
    pub h_ansi: Option<String>,
}

/// `w:hyperlink` element (ECMA-376 §17.16.22).
#[derive(Debug, Clone)]
pub struct DocxHyperlink {
    /// `@r:id` — relationship id for the URL (external hyperlinks).
    pub rel_id: Option<String>,
    /// `@w:anchor` — bookmark anchor within the document.
    pub anchor: Option<String>,
    /// Contained runs.
    pub runs: Vec<DocxRun>,
}

/// An inline drawing from `w:drawing` (ECMA-376 §17.3.3.9).
#[derive(Debug, Clone, Default)]
pub struct DocxDrawing {
    /// The relationship id from `a:blip @r:embed`.
    pub rel_id: Option<String>,
    /// Width in EMUs from `wp:extent @cx`.
    pub cx: Option<i64>,
    /// Height in EMUs from `wp:extent @cy`.
    pub cy: Option<i64>,
    /// Description / alt text from `wp:docPr @descr`.
    pub descr: Option<String>,
    /// Name from `wp:docPr @name`.
    pub name: Option<String>,
    /// Whether this is an anchor (floating) rather than inline drawing.
    pub is_anchor: bool,
    /// Text-wrap configuration for a floating (anchored) drawing.
    /// `None` for inline drawings or anchored drawings without a wrap element.
    pub wrap: Option<loki_doc_model::content::float::FloatWrap>,
    /// `w:txbxContent` paragraphs when this drawing is a `wps` text box
    /// (empty otherwise). Their presence marks the drawing as a text box
    /// rather than a picture.
    pub txbx: Vec<super::DocxParagraph>,
    /// Text-box fill colour from `wps:spPr/a:solidFill/a:srgbClr` (hex, no `#`).
    pub fill_color: Option<String>,
    /// Text-box border colour from `wps:spPr/a:ln/.../a:srgbClr` (hex).
    pub line_color: Option<String>,
    /// Text-box border width in EMUs from `a:ln @w`.
    pub line_w_emu: Option<i64>,
}
