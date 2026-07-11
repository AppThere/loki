// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `w:sectPr` (section properties) → [`DocxSectPr`].
//!
//! ECMA-376 §17.6: page size/margins, header/footer references, columns
//! (including unequal `w:col @w:w` widths, feature 5.10), page numbering, and
//! section start type. Split out of `reader/document.rs` to hold that file's
//! line ceiling.

use quick_xml::{Reader, events::Event};

use crate::docx::model::paragraph::{DocxCols, DocxHdrFtrRef, DocxPgMar, DocxPgSz, DocxSectPr};
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
