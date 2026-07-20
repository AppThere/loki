// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Canonical child-element order for the ordering-sensitive `WordprocessingML`
//! complex types (ECMA-376 Part 1, §17).
//!
//! OOXML schemas define each complex type as an `xsd:sequence`, so a conforming
//! consumer (Microsoft Word) rejects a file whose child elements appear out of
//! order — even though a tolerant, name-matching reader (Loki, `LibreOffice`)
//! loads it fine. These tables give the required order for the containers that
//! most commonly carry hand-authored or tool-emitted violations. Names are
//! **local names** (namespace prefix stripped), matched case-sensitively.

/// The canonical child order for a container `local`, or `None` if the
/// container is not order-checked (its children may appear in any order).
#[must_use]
pub(super) fn schema_order(local: &str) -> Option<&'static [&'static str]> {
    Some(match local {
        "pPr" => &PPR,
        "rPr" => &RPR,
        "sectPr" => &SECT_PR,
        "tcPr" => &TC_PR,
        "tblPr" => &TBL_PR,
        "trPr" => &TR_PR,
        "lvl" => &LVL,
        "style" => &STYLE,
        "abstractNum" => &ABSTRACT_NUM,
        _ => return None,
    })
}

/// `CT_PPr` / `CT_PPrBase` (§17.3.1.26) — paragraph properties.
const PPR: [&str; 36] = [
    "pStyle",
    "keepNext",
    "keepLines",
    "pageBreakBefore",
    "framePr",
    "widowControl",
    "numPr",
    "suppressLineNumbers",
    "pBdr",
    "shd",
    "tabs",
    "suppressAutoHyphens",
    "kinsoku",
    "wordWrap",
    "overflowPunct",
    "topLinePunct",
    "autoSpaceDE",
    "autoSpaceDN",
    "bidi",
    "adjustRightInd",
    "snapToGrid",
    "spacing",
    "ind",
    "contextualSpacing",
    "mirrorIndents",
    "suppressOverlap",
    "jc",
    "textDirection",
    "textAlignment",
    "textboxTightWrap",
    "outlineLvl",
    "divId",
    "cnfStyle",
    "rPr",
    "sectPr",
    "pPrChange",
];

/// `CT_RPr` (§17.3.2.28) — run properties.
const RPR: [&str; 41] = [
    "rStyle",
    "rFonts",
    "b",
    "bCs",
    "i",
    "iCs",
    "caps",
    "smallCaps",
    "strike",
    "dstrike",
    "outline",
    "shadow",
    "emboss",
    "imprint",
    "noProof",
    "snapToGrid",
    "vanish",
    "webHidden",
    "color",
    "spacing",
    "w",
    "kern",
    "position",
    "sz",
    "szCs",
    "highlight",
    "u",
    "effect",
    "bdr",
    "shd",
    "fitText",
    "vertAlign",
    "rtl",
    "cs",
    "em",
    "lang",
    "eastAsianLayout",
    "specVanish",
    "oMath",
    "rPrChange",
    "del",
];

/// `CT_SectPr` (§17.6.17) — section properties.
const SECT_PR: [&str; 21] = [
    "headerReference",
    "footerReference",
    "footnotePr",
    "endnotePr",
    "type",
    "pgSz",
    "pgMar",
    "paperSrc",
    "pgBorders",
    "lnNumType",
    "pgNumType",
    "cols",
    "formProt",
    "vAlign",
    "noEndnote",
    "titlePg",
    "textDirection",
    "bidi",
    "rtlGutter",
    "docGrid",
    "printerSettings",
];

/// `CT_TcPr` (§17.4.70) — table-cell properties.
const TC_PR: [&str; 18] = [
    "cnfStyle",
    "tcW",
    "gridSpan",
    "hMerge",
    "vMerge",
    "tcBorders",
    "shd",
    "noWrap",
    "tcMar",
    "textDirection",
    "tcFitText",
    "vAlign",
    "hideMark",
    "headers",
    "cellIns",
    "cellDel",
    "cellMerge",
    "tcPrChange",
];

/// `CT_TblPr` (§17.4.60) — table properties.
const TBL_PR: [&str; 18] = [
    "tblStyle",
    "tblpPr",
    "tblOverlap",
    "bidiVisual",
    "tblStyleRowBandSize",
    "tblStyleColBandSize",
    "tblW",
    "jc",
    "tblCellSpacing",
    "tblInd",
    "tblBorders",
    "shd",
    "tblLayout",
    "tblCellMar",
    "tblLook",
    "tblCaption",
    "tblDescription",
    "tblPrChange",
];

/// `CT_TrPr` (§17.4.79) — table-row properties.
const TR_PR: [&str; 15] = [
    "cnfStyle",
    "divId",
    "gridBefore",
    "gridAfter",
    "wBefore",
    "wAfter",
    "cantSplit",
    "trHeight",
    "tblHeader",
    "tblCellSpacing",
    "jc",
    "hidden",
    "ins",
    "del",
    "trPrChange",
];

/// `CT_Lvl` (§17.9.6) — a numbering level definition.
const LVL: [&str; 12] = [
    "start",
    "numFmt",
    "lvlRestart",
    "pStyle",
    "isLgl",
    "suff",
    "lvlText",
    "lvlPicBulletId",
    "legacy",
    "lvlJc",
    "pPr",
    "rPr",
];

/// `CT_Style` (§17.7.4.17) — a style definition.
const STYLE: [&str; 22] = [
    "name",
    "aliases",
    "basedOn",
    "next",
    "link",
    "autoRedefine",
    "hidden",
    "uiPriority",
    "semiHidden",
    "unhideWhenUsed",
    "qFormat",
    "locked",
    "personal",
    "personalCompose",
    "personalReply",
    "rsid",
    "pPr",
    "rPr",
    "tblPr",
    "trPr",
    "tcPr",
    "tblStylePr",
];

/// `CT_AbstractNum` (§17.9.1) — an abstract numbering definition.
const ABSTRACT_NUM: [&str; 7] = [
    "nsid",
    "multiLevelType",
    "tmpl",
    "name",
    "styleLink",
    "numStyleLink",
    "lvl",
];
