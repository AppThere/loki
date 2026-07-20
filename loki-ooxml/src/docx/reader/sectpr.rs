// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `w:sectPr` (section properties) → [`DocxSectPr`].
//!
//! ECMA-376 §17.6: page size/margins, header/footer references, columns
//! (including unequal `w:col @w:w` widths, feature 5.10), page numbering, and
//! section start type. Split out of `reader/document.rs` to hold that file's
//! line ceiling.

use quick_xml::{Reader, events::Event};

use crate::docx::model::paragraph::{
    DocxBorderEdge, DocxCols, DocxHdrFtrRef, DocxPgMar, DocxPgSz, DocxSectPr,
};
use crate::docx::model::section::{DocxLnNumType, DocxPgBorders};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses a `w:sectPr` element. Called after Start("sectPr") is consumed.
pub(crate) fn parse_sect_pr(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxSectPr> {
    let mut sect = DocxSectPr::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
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
                            .is_none_or(|v| !matches!(v.as_str(), "0" | "false" | "off"));
                    }
                    b"cols" => {
                        sect.cols = Some(DocxCols {
                            num: attr_val(e, b"num")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1),
                            space: attr_val(e, b"space")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(720),
                            sep: attr_val(e, b"sep")
                                .is_some_and(|v| !matches!(v.as_str(), "0" | "false" | "off")),
                            col_widths: Vec::new(),
                        });
                    }
                    b"col" => {
                        // `w:col @w:w` (twips) — a column width for `equalWidth="0"`.
                        if let Some(cols) = sect.cols.as_mut()
                            && let Some(w) = attr_val(e, b"w").and_then(|v| v.parse().ok())
                        {
                            cols.col_widths.push(w);
                        }
                    }
                    b"pgNumType" => {
                        // ECMA-376 §17.6.12: @w:fmt number format, @w:start restart.
                        sect.pg_num_fmt = attr_val(e, b"fmt");
                        sect.pg_num_start = attr_val(e, b"start").and_then(|v| v.parse().ok());
                    }
                    b"type" => {
                        // ECMA-376 §17.6.22: section start (continuous/next/even/odd).
                        sect.section_type = attr_val(e, b"val");
                    }
                    b"pgBorders" => {
                        let offset_from_text =
                            attr_val(e, b"offsetFrom").as_deref() == Some("text");
                        sect.pg_borders = Some(parse_pg_borders(reader, offset_from_text)?);
                    }
                    b"lnNumType" => {
                        // ECMA-376 §17.6.8: margin line numbering (attribute-only).
                        sect.ln_num_type = Some(DocxLnNumType {
                            count_by: attr_val(e, b"countBy").and_then(|v| v.parse().ok()),
                            start: attr_val(e, b"start").and_then(|v| v.parse().ok()),
                            restart: attr_val(e, b"restart"),
                            distance: attr_val(e, b"distance").and_then(|v| v.parse().ok()),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"sectPr" => {
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

/// Parses a `w:pgBorders` element (four page edges). Called after its Start
/// event; consumes through the matching End. `@w:space` is in points here.
fn parse_pg_borders(
    reader: &mut Reader<&[u8]>,
    offset_from_text: bool,
) -> OoxmlResult<DocxPgBorders> {
    let mut borders = DocxPgBorders {
        offset_from_text,
        ..Default::default()
    };
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
                    b"top" => borders.top = Some(edge),
                    b"bottom" => borders.bottom = Some(edge),
                    b"left" => borders.left = Some(edge),
                    b"right" => borders.right = Some(edge),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"pgBorders" => break,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `parse_sect_pr` over a `w:sectPr` fragment (advancing past its
    /// opening tag first, which the caller normally consumes).
    fn parse(xml: &str) -> DocxSectPr {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Start(ref e) if local_name(e.local_name().as_ref()) == b"sectPr" => break,
                Event::Eof => panic!("no sectPr"),
                _ => {}
            }
        }
        parse_sect_pr(&mut reader).unwrap()
    }

    #[test]
    fn parses_page_borders() {
        let xml = r#"<w:sectPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:pgSz w:w="12240" w:h="15840"/>
          <w:pgBorders w:offsetFrom="page">
            <w:top w:val="single" w:sz="8" w:space="24" w:color="4472C4"/>
            <w:left w:val="single" w:sz="8" w:space="24" w:color="4472C4"/>
            <w:bottom w:val="single" w:sz="8" w:space="24" w:color="4472C4"/>
            <w:right w:val="single" w:sz="8" w:space="24" w:color="4472C4"/>
          </w:pgBorders>
        </w:sectPr>"#;
        let pb = parse(xml).pg_borders.expect("pg_borders parsed");
        assert!(pb.top.is_some() && pb.left.is_some() && pb.bottom.is_some() && pb.right.is_some());
        assert!(!pb.offset_from_text, "offsetFrom=page → not from text");
        let top = pb.top.as_ref().unwrap();
        assert_eq!(top.space, Some(24));
        assert_eq!(top.sz, Some(8));
    }

    #[test]
    fn offset_from_text_is_captured() {
        let xml = r#"<w:sectPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:pgBorders w:offsetFrom="text"><w:top w:val="single" w:sz="4" w:space="1"/></w:pgBorders>
        </w:sectPr>"#;
        let pb = parse(xml).pg_borders.expect("pg_borders");
        assert!(pb.offset_from_text);
    }

    #[test]
    fn parses_line_numbering() {
        let xml = r#"<w:sectPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:lnNumType w:countBy="1" w:start="1" w:restart="newPage"/>
        </w:sectPr>"#;
        let ln = parse(xml).ln_num_type.expect("lnNumType parsed");
        assert_eq!(ln.count_by, Some(1));
        assert_eq!(ln.start, Some(1));
        assert_eq!(ln.restart.as_deref(), Some("newPage"));
        assert_eq!(ln.distance, None);
    }
}
