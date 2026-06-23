// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate model for OOXML paragraph and run elements.
//!
//! Mirrors ECMA-376 §17.3.1 (paragraphs) and §17.3.2 (runs).

pub use super::section::{DocxCols, DocxHdrFtrRef, DocxPgMar, DocxPgSz, DocxSectPr};

/// Intermediate model for `w:p` (ECMA-376 §17.3.1.22).
#[derive(Debug, Clone, Default)]
pub struct DocxParagraph {
    /// Paragraph properties from `w:pPr`.
    pub ppr: Option<DocxPPr>,
    /// Content children: runs, bookmarks, hyperlinks, etc.
    pub children: Vec<DocxParaChild>,
}

/// A child element of `w:p` beyond `w:pPr`.
// Run is substantially larger than Hyperlink; boxing would add indirection on every access.
#[allow(clippy::large_enum_variant, dead_code)]
#[derive(Debug, Clone)]
pub enum DocxParaChild {
    /// A `w:r` text run (ECMA-376 §17.3.2.25).
    Run(DocxRun),
    /// A `w:hyperlink` element (ECMA-376 §17.16.22).
    Hyperlink(DocxHyperlink),
    /// A `w:bookmarkStart` element (ECMA-376 §17.13.6.2).
    BookmarkStart { id: String, name: String },
    /// A `w:bookmarkEnd` element (ECMA-376 §17.13.6.1).
    BookmarkEnd { id: String },
    /// A `w:del` tracked deletion (ECMA-376 §17.13.5.14).
    TrackDel(Vec<DocxRun>),
    /// A `w:ins` tracked insertion (ECMA-376 §17.13.5.16).
    TrackIns(Vec<DocxRun>),
    /// A `w:fldSimple` simple field (ECMA-376 §17.16.19): the `@w:instr`
    /// instruction with the cached result carried as child runs.
    SimpleField {
        /// The field instruction string from `@w:instr`.
        instr: String,
        /// The cached result content (child runs).
        runs: Vec<DocxRun>,
    },
    /// A `w:commentRangeStart` element (ECMA-376 §17.13.4.4).
    CommentRangeStart {
        /// The `@w:id` identifying the comment.
        id: String,
    },
    /// A `w:commentRangeEnd` element (ECMA-376 §17.13.4.3).
    CommentRangeEnd {
        /// The `@w:id` identifying the comment.
        id: String,
    },
    /// An OMML math zone (`m:oMath` inline or `m:oMathPara` display), already
    /// converted to a `MathML` string. ECMA-376 §22.1.
    Math {
        /// The math content as a `MathML` `<math>…</math>` string.
        mathml: String,
        /// `true` when the source was `m:oMathPara` (display/block math).
        display: bool,
    },
}

/// Intermediate model for `w:pPr` (ECMA-376 §17.3.1.26).
#[derive(Debug, Clone, Default)]
pub struct DocxPPr {
    /// Paragraph style id from `w:pStyle @w:val`.
    pub style_id: Option<String>,
    /// Justification from `w:jc @w:val`.
    pub jc: Option<String>,
    /// Indentation from `w:ind`.
    pub ind: Option<DocxInd>,
    /// Spacing from `w:spacing`.
    pub spacing: Option<DocxSpacing>,
    /// Keep lines together (`w:keepLines`).
    pub keep_lines: Option<bool>,
    /// Keep with next paragraph (`w:keepNext`).
    pub keep_next: Option<bool>,
    /// Page break before (`w:pageBreakBefore`).
    pub page_break_before: Option<bool>,
    /// Outline level from `w:outlineLvl @w:val` (0-indexed).
    pub outline_lvl: Option<u8>,
    /// List numbering properties from `w:numPr`.
    pub num_pr: Option<DocxNumPr>,
    /// Tab stop list from `w:tabs`.
    pub tabs: Vec<DocxTab>,
    /// Paragraph borders from `w:pBdr`.
    pub p_bdr: Option<DocxPBdr>,
    /// Section properties from `w:sectPr` (section break mid-document).
    pub sect_pr: Option<DocxSectPr>,
    /// `BiDi` paragraph direction (`w:bidi`).
    pub bidi: Option<bool>,
    /// Widow/orphan control (`w:widowControl`; default on).
    pub widow_control: Option<bool>,
    /// Paragraph shading fill color from `w:shd @w:fill` (hex, no `#`).
    pub shd_fill: Option<String>,
    /// Paragraph-mark run properties from `w:pPr/w:rPr`.
    /// Carries formatting that applies to the paragraph mark itself (e.g. a
    /// font override that affects the default spacing of an empty paragraph).
    pub ppr_rpr: Option<DocxRPr>,
}

/// `w:ind` indentation attributes (ECMA-376 §17.3.1.12).
#[derive(Debug, Clone, Default)]
pub struct DocxInd {
    /// `@w:left` — start indent in twips.
    pub left: Option<i32>,
    /// `@w:right` — end indent in twips.
    pub right: Option<i32>,
    /// `@w:firstLine` — first-line additional indent in twips.
    pub first_line: Option<i32>,
    /// `@w:hanging` — hanging indent in twips.
    pub hanging: Option<i32>,
}

/// `w:spacing` spacing attributes (ECMA-376 §17.3.1.33).
#[derive(Debug, Clone, Default)]
pub struct DocxSpacing {
    /// `@w:before` — space before in twips.
    pub before: Option<i32>,
    /// `@w:after` — space after in twips.
    pub after: Option<i32>,
    /// `@w:line` — line height value.
    pub line: Option<i32>,
    /// `@w:lineRule` — interpretation of `line` (`"auto"`, `"exact"`, `"atLeast"`).
    pub line_rule: Option<String>,
}

/// `w:numPr` numbering properties (ECMA-376 §17.3.1.19).
#[derive(Debug, Clone)]
pub struct DocxNumPr {
    /// `w:ilvl @w:val` — zero-indexed list level.
    pub ilvl: u8,
    /// `w:numId @w:val` — numbering definition instance id.
    pub num_id: u32,
}

/// A single tab stop from `w:tab` in `w:tabs` (ECMA-376 §17.3.1.37).
#[derive(Debug, Clone)]
pub struct DocxTab {
    /// `@w:val` — alignment (`"left"`, `"right"`, `"center"`, `"decimal"`, `"clear"`).
    pub val: String,
    /// `@w:pos` — position in twips.
    pub pos: i32,
    /// `@w:leader` — leader character.
    pub leader: Option<String>,
}

/// `w:pBdr` paragraph border group (ECMA-376 §17.3.1.24).
#[derive(Debug, Clone, Default)]
pub struct DocxPBdr {
    /// `w:top` top border.
    pub top: Option<DocxBorderEdge>,
    /// `w:bottom` bottom border.
    pub bottom: Option<DocxBorderEdge>,
    /// `w:left` start border.
    pub left: Option<DocxBorderEdge>,
    /// `w:right` end border.
    pub right: Option<DocxBorderEdge>,
    /// `w:between` inter-paragraph border.
    pub between: Option<DocxBorderEdge>,
}

/// A single border edge within `w:pBdr` (ECMA-376 §17.3.1.24).
#[derive(Debug, Clone)]
pub struct DocxBorderEdge {
    /// `@w:val` — border style name (`"single"`, `"double"`, `"dashed"`, `"nil"`, etc.).
    pub val: String,
    /// `@w:sz` — border width in eighths of a point.
    pub sz: Option<i32>,
    /// `@w:color` — hex color string (without `#`).
    pub color: Option<String>,
    /// `@w:space` — paragraph-to-border spacing in points.
    pub space: Option<i32>,
}

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
    /// `w:color @w:val` — foreground color hex string.
    pub color: Option<String>,
    /// `w:highlight @w:val` — named highlight color.
    pub highlight: Option<String>,
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
    /// `w:outline` toggle — hollow/outline text effect.
    pub outline: Option<bool>,
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
#[derive(Debug, Clone)]
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
}
