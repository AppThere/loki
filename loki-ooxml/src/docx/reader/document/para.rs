// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Parsers for `w:p` (paragraph) and `w:pPr` (paragraph properties).

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::paragraph::{
    DocxHyperlink, DocxInd, DocxNumPr, DocxPPr, DocxParaChild, DocxParagraph, DocxRun,
    DocxSpacing,
};
use crate::docx::reader::util::{attr_val, local_name, toggle_prop};
use crate::error::{OoxmlError, OoxmlResult};

use super::run::{parse_rpr_element, parse_run};
use super::sect::{parse_pbdr, parse_sect_pr, parse_tabs};

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
        b"shd" => ppr.shd_fill = attr_val(e, b"fill"),
        _ => {}
    }
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
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"hyperlink" => {
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
fn parse_tracked_runs(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> OoxmlResult<Vec<DocxRun>> {
    let mut runs = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"r" => {
                runs.push(parse_run(reader)?);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == end_tag => {
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
