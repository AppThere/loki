// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `word/numbering.xml` → [`DocxNumbering`].
//!
//! ECMA-376 §17.9 (numbering definitions).

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::numbering::{
    DocxAbstractNum, DocxLevel, DocxLvlOverride, DocxNum, DocxNumPicBullet, DocxNumbering,
};
use crate::docx::reader::document::{parse_ppr_element, parse_rpr_element};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses `word/numbering.xml` into a [`DocxNumbering`] model.
///
/// ECMA-376 §17.9.17.
pub fn parse_numbering(xml: &[u8]) -> OoxmlResult<DocxNumbering> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut result = DocxNumbering::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"abstractNum" => {
                    let abstract_num_id = attr_val(e, b"abstractNumId")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(0);
                    let abs = parse_abstract_num(&mut reader, abstract_num_id)?;
                    result.abstract_nums.push(abs);
                }
                b"num" => {
                    let num_id = attr_val(e, b"numId")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(0);
                    let num = parse_num(&mut reader, num_id)?;
                    result.nums.push(num);
                }
                b"numPicBullet" => {
                    let id = attr_val(e, b"numPicBulletId")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(0);
                    if let Some(rel_id) = parse_num_pic_bullet(&mut reader)? {
                        result.pic_bullets.push(DocxNumPicBullet {
                            id,
                            rel_id,
                            src: None,
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/numbering.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(result)
}

/// Scan a `<w:numPicBullet>` for its bullet image relationship id — the first
/// `r:id` on a nested `<v:imagedata>` (or `<a:blip r:embed>`). Returns `None`
/// if no image reference is found.
fn parse_num_pic_bullet(reader: &mut Reader<&[u8]>) -> OoxmlResult<Option<String>> {
    let mut rel_id = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"imagedata" => rel_id = rel_id.or_else(|| attr_val(e, b"id")),
                    b"blip" => rel_id = rel_id.or_else(|| attr_val(e, b"embed")),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"numPicBullet" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/numbering.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(rel_id)
}

fn parse_abstract_num(
    reader: &mut Reader<&[u8]>,
    abstract_num_id: u32,
) -> OoxmlResult<DocxAbstractNum> {
    let mut abs = DocxAbstractNum {
        abstract_num_id,
        levels: Vec::new(),
    };
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"lvl" => {
                let ilvl = attr_val(e, b"ilvl")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(0);
                let level_def = parse_level(reader, ilvl)?;
                abs.levels.push(level_def);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"abstractNum" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/numbering.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(abs)
}

fn parse_num(reader: &mut Reader<&[u8]>, num_id: u32) -> OoxmlResult<DocxNum> {
    let mut abstract_num_id = 0u32;
    let mut level_overrides = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"abstractNumId" => {
                        abstract_num_id = attr_val(e, b"val")
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(0);
                    }
                    b"lvlOverride" => {
                        let ilvl = attr_val(e, b"ilvl")
                            .and_then(|v| v.parse::<u8>().ok())
                            .unwrap_or(0);
                        let ov = parse_lvl_override(reader, ilvl)?;
                        level_overrides.push(ov);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"num" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/numbering.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(DocxNum {
        num_id,
        abstract_num_id,
        level_overrides,
    })
}

fn parse_lvl_override(reader: &mut Reader<&[u8]>, ilvl: u8) -> OoxmlResult<DocxLvlOverride> {
    let mut start_override = None;
    let mut level = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"startOverride" => {
                        start_override = attr_val(e, b"val").and_then(|v| v.parse::<u32>().ok());
                    }
                    b"lvl" => {
                        level = Some(parse_level(reader, ilvl)?);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"lvlOverride" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/numbering.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(DocxLvlOverride {
        ilvl,
        start_override,
        level,
    })
}

fn parse_level(reader: &mut Reader<&[u8]>, ilvl: u8) -> OoxmlResult<DocxLevel> {
    let mut level_out = DocxLevel {
        ilvl,
        start: None,
        num_fmt: None,
        lvl_text: None,
        lvl_jc: None,
        ppr: None,
        rpr: None,
        lvl_pic_bullet_id: None,
    };
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"start" => {
                        level_out.start = attr_val(e, b"val").and_then(|v| v.parse::<u32>().ok());
                    }
                    b"numFmt" => level_out.num_fmt = attr_val(e, b"val"),
                    b"lvlText" => level_out.lvl_text = attr_val(e, b"val"),
                    b"lvlJc" => level_out.lvl_jc = attr_val(e, b"val"),
                    b"lvlPicBulletId" => {
                        level_out.lvl_pic_bullet_id =
                            attr_val(e, b"val").and_then(|v| v.parse::<u32>().ok());
                    }
                    b"pPr" => {
                        level_out.ppr = Some(parse_ppr_element(reader)?);
                        continue;
                    }
                    b"rPr" => {
                        level_out.rpr = Some(parse_rpr_element(reader)?);
                        continue;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"lvl" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/numbering.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(level_out)
}

#[cfg(test)]
#[path = "numbering_tests.rs"]
mod tests;
