// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Reader for `word/numbering.xml` → [`DocxNumbering`].
//!
//! ECMA-376 §17.9 (numbering definitions).

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::docx::model::numbering::{
    DocxAbstractNum, DocxLevel, DocxLvlOverride, DocxNum, DocxNumbering,
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
                let lvl = parse_level(reader, ilvl)?;
                abs.levels.push(lvl);
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"abstractNum" =>
            {
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
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
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

fn parse_lvl_override(
    reader: &mut Reader<&[u8]>,
    ilvl: u8,
) -> OoxmlResult<DocxLvlOverride> {
    let mut start_override = None;
    let mut level = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"startOverride" => {
                        start_override = attr_val(e, b"val")
                            .and_then(|v| v.parse::<u32>().ok());
                    }
                    b"lvl" => {
                        level = Some(parse_level(reader, ilvl)?);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e))
                if local_name(e.local_name().as_ref()) == b"lvlOverride" =>
            {
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
    let mut lvl = DocxLevel {
        ilvl,
        start: None,
        num_fmt: None,
        lvl_text: None,
        lvl_jc: None,
        ppr: None,
        rpr: None,
    };
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"start" => {
                        lvl.start = attr_val(e, b"val")
                            .and_then(|v| v.parse::<u32>().ok());
                    }
                    b"numFmt" => lvl.num_fmt = attr_val(e, b"val"),
                    b"lvlText" => lvl.lvl_text = attr_val(e, b"val"),
                    b"lvlJc" => lvl.lvl_jc = attr_val(e, b"val"),
                    b"pPr" => {
                        lvl.ppr = Some(parse_ppr_element(reader)?);
                        continue;
                    }
                    b"rPr" => {
                        lvl.rpr = Some(parse_rpr_element(reader)?);
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
    Ok(lvl)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_NUMBERING: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0">
      <w:start w:val="1"/>
      <w:numFmt w:val="decimal"/>
      <w:lvlText w:val="%1."/>
      <w:lvlJc w:val="left"/>
    </w:lvl>
    <w:lvl w:ilvl="1">
      <w:start w:val="1"/>
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="&#x2022;"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="0"/>
  </w:num>
</w:numbering>"#;

    #[test]
    fn parses_abstract_num() {
        let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
        assert_eq!(numbering.abstract_nums.len(), 1);
        assert_eq!(numbering.abstract_nums[0].levels.len(), 2);
    }

    #[test]
    fn decimal_level_zero() {
        let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
        let lvl = &numbering.abstract_nums[0].levels[0];
        assert_eq!(lvl.num_fmt.as_deref(), Some("decimal"));
        assert_eq!(lvl.lvl_text.as_deref(), Some("%1."));
    }

    #[test]
    fn resolves_num_to_abstract() {
        let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
        assert_eq!(numbering.abstract_num_id_for(1), Some(0));
    }

    #[test]
    fn three_level_indirection() {
        let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
        let resolved = numbering.resolve(1).unwrap();
        let lvl = resolved.level(0).unwrap();
        assert_eq!(lvl.num_fmt.as_deref(), Some("decimal"));
    }
}
