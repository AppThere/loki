// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `word/document.xml` → [`DocxDocument`].
//!
//! ECMA-376 §17.2 (document structure), §17.3 (block-level content).
//! Uses `quick-xml` event reader with `trim_text(false)` per ADR-0002.

use quick_xml::{Reader, events::Event};

use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::model::paragraph::{DocxHyperlink, DocxParaChild, DocxParagraph};
use crate::docx::reader::runs::{parse_fld_simple_runs, parse_hyperlink_runs, parse_tracked_runs};
use crate::docx::reader::sectpr::parse_sect_pr;
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

#[path = "document_cell.rs"]
mod cell;
#[path = "document_drawing.rs"]
mod drawing;
#[path = "document_ppr.rs"]
mod ppr;
#[path = "document_run.rs"]
mod run;
#[path = "document_sdt.rs"]
mod sdt;
#[path = "document_table.rs"]
mod table;

pub(crate) use drawing::parse_drawing;
pub(crate) use ppr::parse_ppr_element;
pub(crate) use run::{parse_rpr_element, parse_run};

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
                    let tbl = table::parse_table(&mut reader, 0)?;
                    doc.body.children.push(DocxBodyChild::Table(tbl));
                }
                // Unwrap the content control's `w:sdtContent` into the body.
                b"sdt" if in_body => sdt::parse_sdt(&mut reader, &mut doc.body.children, 0)?,
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
