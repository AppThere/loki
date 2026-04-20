// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `word/document.xml` → [`DocxDocument`].
//!
//! ECMA-376 §17.2 (document structure), §17.3 (block-level content).
//! Uses `quick-xml` event reader with `trim_text(false)` per ADR-0002.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::model::paragraph::*;
use crate::docx::model::styles::{DocxTableCell, DocxTableModel, DocxTableRow, DocxTcBorders, DocxTcPr, DocxTrPr};
use crate::docx::reader::util::{attr_val, local_name, parse_emu, toggle_prop};
use crate::error::{OoxmlError, OoxmlResult};

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
                    let tbl = parse_table(&mut reader)?;
                    doc.body.children.push(DocxBodyChild::Table(tbl));
                }
                b"sdt" if in_body => {
                    skip_element(&mut reader, b"sdt")?;
                    doc.body.children.push(DocxBodyChild::Sdt);
                }
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

/// Parses a `w:p` element from the current reader position.
/// Called when the `Start("p")` event has already been consumed.
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
                    b"del" => {
                        depth -= 1;
                        let runs = parse_tracked_runs(reader, b"del")?;
                        para.children.push(DocxParaChild::TrackDel(runs));
                        continue;
                    }
                    b"ins" => {
                        depth -= 1;
                        let runs = parse_tracked_runs(reader, b"ins")?;
                        para.children.push(DocxParaChild::TrackIns(runs));
                        continue;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
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
                    _ => {}
                }
            }
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
            Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"numPr" => {
                        let mut ilvl = 0u8;
                        let mut num_id = 0u32;
                        let mut nbuf = Vec::new();
                        loop {
                            match reader.read_event_into(&mut nbuf) {
                                Ok(Event::Empty(ref ne)) | Ok(Event::Start(ref ne)) => {
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
                        parse_rpr_element(reader)?;
                    }
                    name => apply_ppr_attr(name, e, &mut ppr),
                }
            }
            Ok(Event::Empty(ref e)) => {
                apply_ppr_attr(local_name(e.local_name().as_ref()), e, &mut ppr);
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"pPr" =>
            {
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
                right: attr_val(e, b"right").as_deref().and_then(|v| v.parse().ok()),
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
                before: attr_val(e, b"before").as_deref().and_then(|v| v.parse().ok()),
                after: attr_val(e, b"after").as_deref().and_then(|v| v.parse().ok()),
                line: attr_val(e, b"line").as_deref().and_then(|v| v.parse().ok()),
                line_rule: attr_val(e, b"lineRule"),
            });
        }
        b"keepLines" => ppr.keep_lines = toggle_prop(attr_val(e, b"val").as_deref()),
        b"keepNext" => ppr.keep_next = toggle_prop(attr_val(e, b"val").as_deref()),
        b"pageBreakBefore" => {
            ppr.page_break_before = toggle_prop(attr_val(e, b"val").as_deref());
        }
        b"outlineLvl" => {
            ppr.outline_lvl = attr_val(e, b"val")
                .as_deref()
                .and_then(|v| v.parse::<u8>().ok());
        }
        b"bidi" => ppr.bidi = toggle_prop(attr_val(e, b"val").as_deref()),
        b"widowControl" => ppr.widow_control = toggle_prop(attr_val(e, b"val").as_deref()),
        _ => {}
    }
}

/// Parses a `w:rPr` element. Called after the Start("rPr") event is consumed.
pub(crate) fn parse_rpr_element(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxRPr> {
    let mut rpr = DocxRPr::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"rStyle" => rpr.style_id = attr_val(e, b"val"),
                    b"b" => rpr.bold = toggle_prop(attr_val(e, b"val").as_deref()),
                    b"i" => rpr.italic = toggle_prop(attr_val(e, b"val").as_deref()),
                    b"u" => rpr.underline = attr_val(e, b"val"),
                    b"strike" => {
                        rpr.strike = toggle_prop(attr_val(e, b"val").as_deref());
                    }
                    b"dstrike" => {
                        rpr.dstrike = toggle_prop(attr_val(e, b"val").as_deref());
                    }
                    b"smallCaps" => {
                        rpr.small_caps = toggle_prop(attr_val(e, b"val").as_deref());
                    }
                    b"caps" => {
                        rpr.all_caps = toggle_prop(attr_val(e, b"val").as_deref());
                    }
                    b"shadow" => {
                        rpr.shadow = toggle_prop(attr_val(e, b"val").as_deref());
                    }
                    b"color" => rpr.color = attr_val(e, b"val"),
                    b"highlight" => rpr.highlight = attr_val(e, b"val"),
                    b"sz" => {
                        rpr.sz = attr_val(e, b"val")
                            .as_deref()
                            .and_then(|v| v.parse().ok());
                    }
                    b"szCs" => {
                        rpr.sz_cs = attr_val(e, b"val")
                            .as_deref()
                            .and_then(|v| v.parse().ok());
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
                        rpr.kern = attr_val(e, b"val")
                            .as_deref()
                            .and_then(|v| v.parse().ok());
                    }
                    b"spacing" => {
                        rpr.spacing = attr_val(e, b"val")
                            .as_deref()
                            .and_then(|v| v.parse().ok());
                    }
                    b"w" => {
                        rpr.scale = attr_val(e, b"val")
                            .as_deref()
                            .and_then(|v| v.parse().ok());
                    }
                    b"lang" => rpr.lang = attr_val(e, b"val"),
                    b"vertAlign" => rpr.vert_align = attr_val(e, b"val"),
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"rPr" =>
            {
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
fn parse_run(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxRun> {
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
                b"t" => {
                    let preserve = attr_val(e, b"space")
                        .map_or(false, |v| v == "preserve");
                    let mut text = String::new();
                    let mut tbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut tbuf) {
                            Ok(Event::Text(ref t)) => {
                                if let Ok(s) = t.unescape() {
                                    text.push_str(&s);
                                }
                            }
                            Ok(Event::End(ref et))
                                if local_name(et.local_name().as_ref()) == b"t" =>
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
                            Ok(Event::Text(ref t)) => {
                                if let Ok(s) = t.unescape() {
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
            Ok(Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"br" => {
                        let break_type = attr_val(e, b"type");
                        run.children.push(DocxRunChild::Break { break_type });
                    }
                    b"tab" => {
                        run.children.push(DocxRunChild::Tab);
                    }
                    b"fldChar" => {
                        if let Some(ft) = attr_val(e, b"fldCharType") {
                            run.children.push(DocxRunChild::FldChar {
                                fld_char_type: ft,
                            });
                        }
                    }
                    b"footnoteReference" => {
                        if let Some(id) = attr_val(e, b"id")
                            .and_then(|v| v.parse::<i32>().ok())
                        {
                            run.children.push(DocxRunChild::FootnoteRef { id });
                        }
                    }
                    b"endnoteReference" => {
                        if let Some(id) = attr_val(e, b"id")
                            .and_then(|v| v.parse::<i32>().ok())
                        {
                            run.children.push(DocxRunChild::EndnoteRef { id });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"r" =>
            {
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

/// Parses the runs inside a `w:hyperlink` element.
fn parse_hyperlink_runs(reader: &mut Reader<&[u8]>) -> OoxmlResult<Vec<DocxRun>> {
    let mut runs = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"r" => {
                runs.push(parse_run(reader)?);
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"hyperlink" =>
            {
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
    Ok(runs)
}

/// Consumes runs inside a `w:del` or `w:ins` element.
fn parse_tracked_runs(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
) -> OoxmlResult<Vec<DocxRun>> {
    let mut runs = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"r" => {
                runs.push(parse_run(reader)?);
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == end_tag =>
            {
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
    Ok(runs)
}

/// Parses a `w:pBdr` element. Called after Start("pBdr") is consumed.
fn parse_pbdr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxPBdr> {
    let mut pbdr = DocxPBdr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
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
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"pBdr" =>
            {
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
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e))
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
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"tabs" =>
            {
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

/// Parses a `w:sectPr` element. Called after Start("sectPr") is consumed.
pub(crate) fn parse_sect_pr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxSectPr> {
    let mut sect = DocxSectPr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"pgSz" => {
                        sect.pg_sz = Some(DocxPgSz {
                            w: attr_val(e, b"w")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(12240),
                            h: attr_val(e, b"h")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(15840),
                            orient: attr_val(e, b"orient"),
                        });
                    }
                    b"pgMar" => {
                        sect.pg_mar = Some(DocxPgMar {
                            top: attr_val(e, b"top")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            bottom: attr_val(e, b"bottom")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            left: attr_val(e, b"left")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            right: attr_val(e, b"right")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1440),
                            header: attr_val(e, b"header")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(720),
                            footer: attr_val(e, b"footer")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(720),
                            gutter: attr_val(e, b"gutter")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                        });
                    }
                    b"headerReference" => {
                        if let (Some(hf_type), Some(rel_id)) =
                            (attr_val(e, b"type"), attr_val(e, b"id"))
                        {
                            sect.header_refs.push(DocxHdrFtrRef { hf_type, rel_id });
                        }
                    }
                    b"footerReference" => {
                        if let (Some(hf_type), Some(rel_id)) =
                            (attr_val(e, b"type"), attr_val(e, b"id"))
                        {
                            sect.footer_refs.push(DocxHdrFtrRef { hf_type, rel_id });
                        }
                    }
                    b"titlePg" => {
                        // Presence enables first-page variant; w:val="0" disables.
                        sect.title_page = attr_val(e, b"val")
                            .map_or(true, |v| !matches!(v.as_str(), "0" | "false" | "off"));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"sectPr" =>
            {
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
    Ok(sect)
}

/// Parses a `w:drawing` element. Called after Start("drawing") is consumed.
fn parse_drawing(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxDrawing> {
    let mut drawing = DocxDrawing {
        rel_id: None,
        cx: None,
        cy: None,
        descr: None,
        name: None,
        is_anchor: false,
    };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"anchor" => drawing.is_anchor = true,
                    b"extent" => {
                        drawing.cx = attr_val(e, b"cx")
                            .as_deref()
                            .and_then(parse_emu);
                        drawing.cy = attr_val(e, b"cy")
                            .as_deref()
                            .and_then(parse_emu);
                    }
                    b"docPr" => {
                        drawing.descr = attr_val(e, b"descr");
                        drawing.name = attr_val(e, b"name");
                    }
                    b"blip" => {
                        drawing.rel_id = attr_val(e, b"embed");
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"drawing" =>
            {
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
    Ok(drawing)
}

/// Parses a `w:tbl` element. Called after Start("tbl") is consumed.
fn parse_table(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableModel> {
    let mut tbl = DocxTableModel::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tr" => {
                    let row = parse_table_row(reader)?;
                    tbl.rows.push(row);
                }
                b"tblGrid" => {} // handle below
                b"gridCol" => {
                    // parse later
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) if local_name(e.local_name().as_ref()) == b"gridCol" => {
                let w: i32 = attr_val(e, b"w")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                tbl.col_widths.push(w);
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"tbl" =>
            {
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
    Ok(tbl)
}

/// Parses a `w:tr` element. Called after Start("tr") is consumed.
fn parse_table_row(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableRow> {
    let mut row = DocxTableRow::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tc" => {
                    let cell = parse_table_cell(reader)?;
                    row.cells.push(cell);
                }
                b"trPr" => {
                    let mut tr_pr = DocxTrPr::default();
                    let mut tbuf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut tbuf) {
                            Ok(Event::Empty(ref te))
                                if local_name(te.local_name().as_ref()) == b"tblHeader" =>
                            {
                                tr_pr.is_header = true;
                            }
                            Ok(Event::End(ref te))
                                if local_name(te.local_name().as_ref()) == b"trPr" =>
                            {
                                break;
                            }
                            Ok(Event::Eof) | Err(_) => break,
                            _ => {}
                        }
                        tbuf.clear();
                    }
                    row.tr_pr = Some(tr_pr);
                }
                _ => {}
            },
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"tr" =>
            {
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
    Ok(row)
}

/// Parses a `w:tc` element. Called after Start("tc") is consumed.
fn parse_table_cell(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTableCell> {
    let mut cell = DocxTableCell::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"tcPr" => {
                    cell.tc_pr = Some(parse_tc_pr(reader)?);
                }
                b"p" => {
                    let para = parse_paragraph(reader)?;
                    cell.paragraphs.push(para);
                }
                _ => {}
            },
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"tc" =>
            {
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
    Ok(cell)
}

/// Parses a `w:tcPr` element. Called after Start("tcPr") is consumed.
fn parse_tc_pr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTcPr> {
    let mut tc_pr = DocxTcPr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"gridSpan" => {
                        tc_pr.grid_span = attr_val(e, b"val").and_then(|v| v.parse().ok());
                    }
                    b"vMerge" => {
                        use crate::docx::model::styles::DocxVMerge;
                        tc_pr.v_merge = Some(
                            if attr_val(e, b"val").as_deref() == Some("restart") {
                                DocxVMerge::Restart
                            } else {
                                DocxVMerge::Continue
                            },
                        );
                    }
                    b"shd" => {
                        tc_pr.shd_fill = attr_val(e, b"fill");
                    }
                    b"tcBorders" => {
                        tc_pr.tc_borders = Some(parse_tc_borders(reader)?);
                        // parse_tc_borders consumes until </tcBorders>, so skip
                        // the fallthrough End event that would match here.
                        buf.clear();
                        continue;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"tcPr" =>
            {
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
    Ok(tc_pr)
}

/// Parses a `w:tcBorders` element. Called after Start("tcBorders") is consumed.
fn parse_tc_borders(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxTcBorders> {
    let mut borders = DocxTcBorders::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let edge = DocxBorderEdge {
                    val: attr_val(e, b"val").unwrap_or_default(),
                    sz: attr_val(e, b"sz").and_then(|v| v.parse().ok()),
                    color: attr_val(e, b"color"),
                    space: attr_val(e, b"space").and_then(|v| v.parse().ok()),
                };
                match local_name(e.local_name().as_ref()) {
                    b"top" => borders.top = Some(edge),
                    b"bottom" => borders.bottom = Some(edge),
                    b"left" | b"start" => borders.left = Some(edge),
                    b"right" | b"end" => borders.right = Some(edge),
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"tcBorders" =>
            {
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
    Ok(borders)
}

/// Skips all content inside an element until its matching end tag.
pub(crate) fn skip_element(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
) -> OoxmlResult<()> {
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
mod tests {
    use super::*;

    const SIMPLE_DOC: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/><w:outlineLvl w:val="0"/></w:pPr>
      <w:r><w:t>Hello</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t xml:space="preserve">World </w:t></w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"
               w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#;

    #[test]
    fn parses_two_paragraphs() {
        let doc = parse_document(SIMPLE_DOC).unwrap();
        let paras: Vec<_> = doc
            .body
            .children
            .iter()
            .filter(|c| matches!(c, DocxBodyChild::Paragraph(_)))
            .collect();
        assert_eq!(paras.len(), 2);
    }

    #[test]
    fn first_para_has_style() {
        let doc = parse_document(SIMPLE_DOC).unwrap();
        if let Some(DocxBodyChild::Paragraph(p)) = doc.body.children.first() {
            assert_eq!(
                p.ppr.as_ref().and_then(|ppr| ppr.style_id.as_deref()),
                Some("Heading1")
            );
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn final_sect_pr_parsed() {
        let doc = parse_document(SIMPLE_DOC).unwrap();
        let sect = doc.body.final_sect_pr.unwrap();
        let pg_sz = sect.pg_sz.unwrap();
        assert_eq!(pg_sz.w, 12240);
        assert_eq!(pg_sz.h, 15840);
    }
}
