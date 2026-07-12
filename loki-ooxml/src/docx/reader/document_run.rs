// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Run and run-property reading for `word/document.xml` (split from
//! `document.rs` for the 300-line ceiling): `parse_rpr_element` reads a
//! `w:rPr` into a [`DocxRPr`], and `parse_run` reads a `w:r` (text, breaks,
//! tabs, drawings, fields, footnote/endnote refs) into a [`DocxRun`]. Both are
//! re-exported from `document.rs` (used by the styles / numbering / runs
//! readers). `parse_drawing` stays in `document.rs`.

use quick_xml::{Reader, events::Event};

use crate::docx::model::paragraph::{DocxRFonts, DocxRPr, DocxRun, DocxRunChild};
use crate::docx::reader::runs::parse_mark_revision;
use crate::docx::reader::util::{attr_val, local_name, toggle_prop};
use crate::error::{OoxmlError, OoxmlResult};
use crate::xml_util::event_text;

use super::parse_drawing;

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
                        rpr.mark_rev = Some(parse_mark_revision(n, e));
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
