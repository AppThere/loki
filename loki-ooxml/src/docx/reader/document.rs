// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `word/document.xml` → [`DocxDocument`].
//!
//! ECMA-376 §17.2 (document structure), §17.3 (block-level content).
//! Uses `quick-xml` event reader with `trim_text(false)` per ADR-0002.

use quick_xml::{Reader, events::Event};

use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::model::paragraph::{
    DocxBorderEdge, DocxDrawing, DocxFramePr, DocxHyperlink, DocxInd, DocxNumPr, DocxPBdr, DocxPPr,
    DocxParaChild, DocxParagraph, DocxRFonts, DocxRPr, DocxRun, DocxRunChild, DocxSpacing, DocxTab,
};
use crate::docx::reader::runs::{parse_fld_simple_runs, parse_hyperlink_runs, parse_tracked_runs};
use crate::docx::reader::sectpr::parse_sect_pr;
use crate::docx::reader::util::{attr_val, local_name, parse_emu, toggle_prop};
use crate::error::{OoxmlError, OoxmlResult};
use crate::xml_util::event_text;

#[path = "document_cell.rs"]
mod cell;
#[path = "document_sdt.rs"]
mod sdt;
#[path = "document_table.rs"]
mod table;
use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

/// Parses `word/document.xml` bytes into a [`DocxDocument`].
pub fn parse_document(xml: &[u8]) -> OoxmlResult<DocxDocument> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut doc = DocxDocument::default();
    let mut in_body = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"body" => in_body = true,
                b"p" if in_body => {
                    let para = parse_paragraph(&mut reader)?;
                    // Check if the paragraph's pPr has a sectPr (section break)
                    doc.body.children.push(DocxBodyChild::Paragraph(para));
                }
                b"tbl" if in_body => {
                    let tbl = table::parse_table(&mut reader)?;
                    doc.body.children.push(DocxBodyChild::Table(tbl));
                }
                // Unwrap the content control's `w:sdtContent` into the body.
                b"sdt" if in_body => sdt::parse_sdt(&mut reader, &mut doc.body.children)?,
                b"sectPr" if in_body => {
                    let sect = parse_sect_pr(&mut reader)?;
                    doc.body.final_sect_pr = Some(sect);
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => {
                if local_name(e.local_name().as_ref()) == b"body" {
                    in_body = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(doc)
}

/// Parses a `w:p` element; called after `Start("p")` has been consumed.
pub(crate) fn parse_paragraph(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxParagraph> {
    let mut para = DocxParagraph::default();
    let mut buf = Vec::new();
    let mut depth = 1i32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                match local_name(e.local_name().as_ref()) {
                    b"pPr" => {
                        depth -= 1; // parse_ppr_element will consume the end tag
                        para.ppr = Some(parse_ppr_element(reader)?);
                        continue;
                    }
                    b"r" => {
                        depth -= 1;
                        let run = parse_run(reader)?;
                        para.children.push(DocxParaChild::Run(run));
                        continue;
                    }
                    b"hyperlink" => {
                        depth -= 1;
                        let rel_id = attr_val(e, b"id");
                        let anchor = attr_val(e, b"anchor");
                        let runs = parse_hyperlink_runs(reader)?;
                        para.children.push(DocxParaChild::Hyperlink(DocxHyperlink {
                            rel_id,
                            anchor,
                            runs,
                        }));
                        continue;
                    }
                    b"bookmarkStart" => {
                        let id = attr_val(e, b"id").unwrap_or_default();
                        let name = attr_val(e, b"name").unwrap_or_default();
                        para.children
                            .push(DocxParaChild::BookmarkStart { id, name });
                    }
                    b"commentRangeStart" => {
                        let id = attr_val(e, b"id").unwrap_or_default();
                        para.children.push(DocxParaChild::CommentRangeStart { id });
                    }
                    b"commentRangeEnd" => {
                        let id = attr_val(e, b"id").unwrap_or_default();
                        para.children.push(DocxParaChild::CommentRangeEnd { id });
                    }
                    b"del" => {
                        depth -= 1;
                        let change = parse_tracked_runs(reader, e, b"del")?;
                        para.children.push(DocxParaChild::TrackDel(change));
                        continue;
                    }
                    b"ins" => {
                        depth -= 1;
                        let change = parse_tracked_runs(reader, e, b"ins")?;
                        para.children.push(DocxParaChild::TrackIns(change));
                        continue;
                    }
                    b"fldSimple" => {
                        depth -= 1;
                        let instr = attr_val(e, b"instr").unwrap_or_default();
                        let runs = parse_fld_simple_runs(reader)?;
                        para.children
                            .push(DocxParaChild::SimpleField { instr, runs });
                        continue;
                    }
                    b"oMath" | b"oMathPara" => {
                        depth -= 1; // read_math consumes the element's end tag
                        let (mathml, display) = crate::docx::omml::read_math(reader, e)?;
                        para.children.push(DocxParaChild::Math { mathml, display });
                        continue;
                    }
                    // Inline content control: its `w:sdtContent` runs flow
                    // through this dispatch (implicit unwrap); skip the chrome
                    // wholesale so `w:sdtPr` internals never leak here (5.9).
                    b"sdtPr" | b"sdtEndPr" => {
                        depth -= 1;
                        let name = local_name(e.local_name().as_ref()).to_vec();
                        skip_element(reader, &name)?;
                        continue;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => match local_name(e.local_name().as_ref()) {
                b"bookmarkStart" => {
                    let id = attr_val(e, b"id").unwrap_or_default();
                    let name = attr_val(e, b"name").unwrap_or_default();
                    para.children
                        .push(DocxParaChild::BookmarkStart { id, name });
                }
                b"bookmarkEnd" => {
                    let id = attr_val(e, b"id").unwrap_or_default();
                    para.children.push(DocxParaChild::BookmarkEnd { id });
                }
                b"commentRangeStart" => {
                    let id = attr_val(e, b"id").unwrap_or_default();
                    para.children.push(DocxParaChild::CommentRangeStart { id });
                }
                b"commentRangeEnd" => {
                    let id = attr_val(e, b"id").unwrap_or_default();
                    para.children.push(DocxParaChild::CommentRangeEnd { id });
                }
                b"fldSimple" => {
                    // Self-closing simple field: instruction only, no cached result.
                    let instr = attr_val(e, b"instr").unwrap_or_default();
                    para.children.push(DocxParaChild::SimpleField {
                        instr,
                        runs: Vec::new(),
                    });
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0 && local_name(e.local_name().as_ref()) == b"p" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(para)
}

/// Parses a `w:pPr` element. Called after the Start("pPr") event is consumed.
pub(crate) fn parse_ppr_element(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxPPr> {
    let mut ppr = DocxPPr::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"numPr" => {
                    let mut ilvl = 0u8;
                    let mut num_id = 0u32;
                    let mut nbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut nbuf) {
                            Ok(Event::Empty(ref ne) | Event::Start(ref ne)) => {
                                match local_name(ne.local_name().as_ref()) {
                                    b"ilvl" => {
                                        ilvl = attr_val(ne, b"val")
                                            .as_deref()
                                            .and_then(|v| v.parse().ok())
                                            .unwrap_or(0);
                                    }
                                    b"numId" => {
                                        num_id = attr_val(ne, b"val")
                                            .as_deref()
                                            .and_then(|v| v.parse().ok())
                                            .unwrap_or(0);
                                    }
                                    _ => {}
                                }
                            }
                            Ok(Event::End(ref ne))
                                if local_name(ne.local_name().as_ref()) == b"numPr" =>
                            {
                                break;
                            }
                            Ok(Event::Eof) | Err(_) => break,
                            _ => {}
                        }
                        nbuf.clear();
                    }
                    ppr.num_pr = Some(DocxNumPr { ilvl, num_id });
                }
                b"pBdr" => {
                    ppr.p_bdr = Some(parse_pbdr(reader)?);
                }
                b"tabs" => {
                    parse_tabs(reader, &mut ppr.tabs)?;
                }
                b"sectPr" => {
                    ppr.sect_pr = Some(parse_sect_pr(reader)?);
                }
                b"rPr" => {
                    ppr.ppr_rpr = Some(parse_rpr_element(reader)?);
                }
                name => apply_ppr_attr(name, e, &mut ppr),
            },
            Ok(Event::Empty(ref e)) => {
                apply_ppr_attr(local_name(e.local_name().as_ref()), e, &mut ppr);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"pPr" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(ppr)
}

/// Applies a single `w:pPr` attribute-only child element to `ppr`.
fn apply_ppr_attr(name: &[u8], e: &quick_xml::events::BytesStart<'_>, ppr: &mut DocxPPr) {
    match name {
        b"pStyle" => ppr.style_id = attr_val(e, b"val"),
        b"jc" => ppr.jc = attr_val(e, b"val"),
        b"ind" => {
            ppr.ind = Some(DocxInd {
                left: attr_val(e, b"left").as_deref().and_then(|v| v.parse().ok()),
                right: attr_val(e, b"right")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                first_line: attr_val(e, b"firstLine")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                hanging: attr_val(e, b"hanging")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
            });
        }
        b"spacing" => {
            ppr.spacing = Some(DocxSpacing {
                before: attr_val(e, b"before")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                after: attr_val(e, b"after")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                line: attr_val(e, b"line").as_deref().and_then(|v| v.parse().ok()),
                line_rule: attr_val(e, b"lineRule"),
            });
        }
        b"keepLines" => ppr.keep_lines = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"keepNext" => ppr.keep_next = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"pageBreakBefore" => {
            ppr.page_break_before = Some(toggle_prop(attr_val(e, b"val").as_deref()));
        }
        b"outlineLvl" => {
            ppr.outline_lvl = attr_val(e, b"val")
                .as_deref()
                .and_then(|v| v.parse::<u8>().ok());
        }
        b"bidi" => ppr.bidi = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"widowControl" => ppr.widow_control = Some(toggle_prop(attr_val(e, b"val").as_deref())),
        b"shd" => {
            ppr.shd_fill = attr_val(e, b"fill");
            ppr.shd_val = attr_val(e, b"val");
            ppr.shd_color = attr_val(e, b"color");
        }
        b"framePr" => {
            ppr.frame_pr = Some(DocxFramePr {
                drop_cap: attr_val(e, b"dropCap"),
                lines: attr_val(e, b"lines")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
                h_space: attr_val(e, b"hSpace")
                    .as_deref()
                    .and_then(|v| v.parse().ok()),
            });
        }
        _ => {}
    }
}

/// Parses a `w:rPr` element. Called after the Start("rPr") event is consumed.
pub(crate) fn parse_rpr_element(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxRPr> {
    let mut rpr = DocxRPr::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"rStyle" => rpr.style_id = attr_val(e, b"val"),
                    b"b" => rpr.bold = Some(toggle_prop(attr_val(e, b"val").as_deref())),
                    b"i" => rpr.italic = Some(toggle_prop(attr_val(e, b"val").as_deref())),
                    b"u" => rpr.underline = attr_val(e, b"val"),
                    b"strike" => {
                        rpr.strike = Some(toggle_prop(attr_val(e, b"val").as_deref()));
                    }
                    b"dstrike" => {
                        rpr.dstrike = Some(toggle_prop(attr_val(e, b"val").as_deref()));
                    }
                    b"smallCaps" => {
                        rpr.small_caps = Some(toggle_prop(attr_val(e, b"val").as_deref()));
                    }
                    b"caps" => {
                        rpr.all_caps = Some(toggle_prop(attr_val(e, b"val").as_deref()));
                    }
                    b"shadow" => {
                        rpr.shadow = Some(toggle_prop(attr_val(e, b"val").as_deref()));
                    }
                    b"color" => rpr.color = attr_val(e, b"val"),
                    b"highlight" => rpr.highlight = attr_val(e, b"val"),
                    b"position" => {
                        rpr.position = attr_val(e, b"val").as_deref().and_then(|v| v.parse().ok());
                    }
                    b"sz" => {
                        rpr.sz = attr_val(e, b"val").as_deref().and_then(|v| v.parse().ok());
                    }
                    b"szCs" => {
                        rpr.sz_cs = attr_val(e, b"val").as_deref().and_then(|v| v.parse().ok());
                    }
                    b"rFonts" => {
                        rpr.fonts = Some(DocxRFonts {
                            ascii: attr_val(e, b"ascii"),
                            cs: attr_val(e, b"cs"),
                            east_asia: attr_val(e, b"eastAsia"),
                            h_ansi: attr_val(e, b"hAnsi"),
                        });
                    }
                    b"kern" => {
                        rpr.kern = attr_val(e, b"val").as_deref().and_then(|v| v.parse().ok());
                    }
                    b"spacing" => {
                        rpr.spacing = attr_val(e, b"val").as_deref().and_then(|v| v.parse().ok());
                    }
                    b"w" => {
                        rpr.scale = attr_val(e, b"val").as_deref().and_then(|v| v.parse().ok());
                    }
                    b"lang" => {
                        rpr.lang = attr_val(e, b"val");
                        rpr.lang_complex = attr_val(e, b"bidi");
                        rpr.lang_east_asian = attr_val(e, b"eastAsia");
                    }
                    b"vertAlign" => rpr.vert_align = attr_val(e, b"val"),
                    b"shd" => {
                        rpr.shd_fill = attr_val(e, b"fill");
                        rpr.shd_val = attr_val(e, b"val");
                        rpr.shd_color = attr_val(e, b"color");
                    }
                    b"outline" => {
                        rpr.outline = Some(toggle_prop(attr_val(e, b"val").as_deref()));
                    }
                    // A tracked ¶ deletion/insertion on a paragraph mark's rPr.
                    n @ (b"del" | b"ins") => {
                        rpr.mark_rev = Some(super::runs::parse_mark_revision(n, e));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"rPr" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(rpr)
}

/// Parses a `w:r` element. Called after the Start("r") event is consumed.
// Function body is a single large match over XML events; splitting would reduce readability.
#[allow(clippy::too_many_lines)]
pub(crate) fn parse_run(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxRun> {
    let mut run = DocxRun::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"rPr" => {
                    run.rpr = Some(parse_rpr_element(reader)?);
                }
                b"drawing" => {
                    let drawing = parse_drawing(reader)?;
                    run.children.push(DocxRunChild::Drawing(drawing));
                }
                tag @ (b"t" | b"delText") => {
                    let preserve = attr_val(e, b"space").is_some_and(|v| v == "preserve");
                    let mut text = String::new();
                    let mut tbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut tbuf) {
                            Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) => {
                                if let Ok(s) = event_text(ev) {
                                    text.push_str(&s);
                                }
                            }
                            Ok(Event::End(ref et))
                                if local_name(et.local_name().as_ref()) == tag =>
                            {
                                break;
                            }
                            Ok(Event::Eof) | Err(_) => break,
                            _ => {}
                        }
                        tbuf.clear();
                    }
                    run.children.push(DocxRunChild::Text { text, preserve });
                    continue;
                }
                b"instrText" => {
                    let mut text = String::new();
                    let mut tbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut tbuf) {
                            Ok(ref ev @ (Event::Text(_) | Event::GeneralRef(_))) => {
                                if let Ok(s) = event_text(ev) {
                                    text.push_str(&s);
                                }
                            }
                            Ok(Event::End(ref et))
                                if local_name(et.local_name().as_ref()) == b"instrText" =>
                            {
                                break;
                            }
                            Ok(Event::Eof) | Err(_) => break,
                            _ => {}
                        }
                        tbuf.clear();
                    }
                    run.children.push(DocxRunChild::InstrText { text });
                    continue;
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) => match local_name(e.local_name().as_ref()) {
                b"br" => {
                    let break_type = attr_val(e, b"type");
                    run.children.push(DocxRunChild::Break { break_type });
                }
                b"tab" => {
                    run.children.push(DocxRunChild::Tab);
                }
                b"fldChar" => {
                    if let Some(ft) = attr_val(e, b"fldCharType") {
                        run.children
                            .push(DocxRunChild::FldChar { fld_char_type: ft });
                    }
                }
                b"footnoteReference" => {
                    if let Some(id) = attr_val(e, b"id").and_then(|v| v.parse::<i32>().ok()) {
                        run.children.push(DocxRunChild::FootnoteRef { id });
                    }
                }
                b"endnoteReference" => {
                    if let Some(id) = attr_val(e, b"id").and_then(|v| v.parse::<i32>().ok()) {
                        run.children.push(DocxRunChild::EndnoteRef { id });
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"r" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(run)
}

/// Parses a `w:pBdr` element. Called after Start("pBdr") is consumed.
fn parse_pbdr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxPBdr> {
    let mut pbdr = DocxPBdr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let edge = DocxBorderEdge {
                    val: attr_val(e, b"val").unwrap_or_default(),
                    sz: attr_val(e, b"sz").and_then(|v| v.parse().ok()),
                    color: attr_val(e, b"color"),
                    space: attr_val(e, b"space").and_then(|v| v.parse().ok()),
                };
                match local_name(e.local_name().as_ref()) {
                    b"top" => pbdr.top = Some(edge),
                    b"bottom" => pbdr.bottom = Some(edge),
                    b"left" => pbdr.left = Some(edge),
                    b"right" => pbdr.right = Some(edge),
                    b"between" => pbdr.between = Some(edge),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"pBdr" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(pbdr)
}

/// Parses a `w:tabs` element and appends each `w:tab` to `out`.
/// Called after Start("tabs") is consumed.
fn parse_tabs(reader: &mut Reader<&[u8]>, out: &mut Vec<DocxTab>) -> OoxmlResult<()> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e))
                if local_name(e.local_name().as_ref()) == b"tab" =>
            {
                if let Some(val) = attr_val(e, b"val") {
                    out.push(DocxTab {
                        val,
                        pos: attr_val(e, b"pos")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0),
                        leader: attr_val(e, b"leader"),
                    });
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"tabs" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// Parses a `w:drawing` element. Called after Start("drawing") is consumed.
fn parse_drawing(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxDrawing> {
    let mut drawing = DocxDrawing::default();
    // Wrap mode/side are carried on a `wp:wrap*` child; `behindDoc` lives on the
    // `wp:anchor` element. Collect both, then assemble the `FloatWrap` at the end.
    let mut wrap_mode: Option<TextWrap> = None;
    let mut wrap_side = WrapSide::Both;
    let mut behind_doc = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"anchor" => {
                        drawing.is_anchor = true;
                        behind_doc = attr_val(e, b"behindDoc").as_deref() == Some("1");
                    }
                    b"extent" => {
                        drawing.cx = attr_val(e, b"cx").as_deref().and_then(parse_emu);
                        drawing.cy = attr_val(e, b"cy").as_deref().and_then(parse_emu);
                    }
                    b"docPr" => {
                        drawing.descr = attr_val(e, b"descr");
                        drawing.name = attr_val(e, b"name");
                    }
                    b"blip" => {
                        drawing.rel_id = attr_val(e, b"embed");
                    }
                    b"wrapSquare" => {
                        wrap_mode = Some(TextWrap::Square);
                        wrap_side = parse_wrap_text(e);
                    }
                    b"wrapTight" => {
                        wrap_mode = Some(TextWrap::Tight);
                        wrap_side = parse_wrap_text(e);
                    }
                    b"wrapThrough" => {
                        wrap_mode = Some(TextWrap::Through);
                        wrap_side = parse_wrap_text(e);
                    }
                    b"wrapTopAndBottom" => wrap_mode = Some(TextWrap::TopAndBottom),
                    b"wrapNone" => wrap_mode = Some(TextWrap::None),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"drawing" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    if let Some(wrap) = wrap_mode {
        drawing.wrap = Some(FloatWrap {
            wrap,
            side: wrap_side,
            behind_text: behind_doc,
        });
    }
    Ok(drawing)
}

/// Reads the `wrapText` attribute of a `wp:wrap*` element into a [`WrapSide`].
fn parse_wrap_text(e: &quick_xml::events::BytesStart<'_>) -> WrapSide {
    match attr_val(e, b"wrapText").as_deref() {
        Some("left") => WrapSide::Left,
        Some("right") => WrapSide::Right,
        Some("largest") => WrapSide::Largest,
        _ => WrapSide::Both,
    }
}

/// Skips all content inside an element until its matching end tag.
pub(crate) fn skip_element(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> OoxmlResult<()> {
    let mut depth = 1i32;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                depth -= 1;
                if depth == 0 && local_name(e.local_name().as_ref()) == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
