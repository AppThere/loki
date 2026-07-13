// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate model for OOXML paragraph and run elements.
//!
//! Mirrors ECMA-376 §17.3.1 (paragraphs) and §17.3.2 (runs).

pub use super::revision::{DocxMarkRevision, DocxRevisionInfo, DocxTrackedChange};
pub use super::section::{DocxCols, DocxHdrFtrRef, DocxPgMar, DocxPgSz, DocxSectPr};

#[path = "paragraph_run.rs"]
mod run;
pub use run::{DocxDrawing, DocxHyperlink, DocxRFonts, DocxRPr, DocxRun, DocxRunChild};

/// Intermediate model for `w:p` (ECMA-376 §17.3.1.22).
#[derive(Debug, Clone, Default)]
pub struct DocxParagraph {
    /// Paragraph properties from `w:pPr`.
    pub ppr: Option<DocxPPr>,
    /// Content children: runs, bookmarks, hyperlinks, etc.
    pub children: Vec<DocxParaChild>,
}

/// A child element of `w:p` beyond `w:pPr`. (`Run` ≫ `Hyperlink`; boxing would add indirection, so the size is allowed.)
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
    TrackDel(DocxTrackedChange),
    /// A `w:ins` tracked insertion (ECMA-376 §17.13.5.16).
    TrackIns(DocxTrackedChange),
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
    /// Paragraph shading pattern from `w:shd @w:val` (e.g. `clear`, `pct25`).
    pub shd_val: Option<String>,
    /// Paragraph shading pattern foreground from `w:shd @w:color` (hex).
    pub shd_color: Option<String>,
    /// Paragraph-mark run properties from `w:pPr/w:rPr` — formatting that applies
    /// to the paragraph mark itself (and its tracked ¶ deletion).
    pub ppr_rpr: Option<DocxRPr>,
    /// Text-frame properties from `w:framePr` — carries drop-cap settings.
    pub frame_pr: Option<DocxFramePr>,
}

/// `w:framePr` text-frame properties (ECMA-376 §17.3.1.11).
///
/// Only the drop-cap-relevant attributes are captured; the full text-frame
/// positioning model is not yet imported.
#[derive(Debug, Clone, Default)]
pub struct DocxFramePr {
    /// `@w:dropCap` — `"drop"`, `"margin"`, `"none"`, or `"default"`.
    pub drop_cap: Option<String>,
    /// `@w:lines` — number of lines the dropped cap spans.
    pub lines: Option<u8>,
    /// `@w:hSpace` — horizontal distance from the surrounding text, in twips.
    pub h_space: Option<i32>,
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
