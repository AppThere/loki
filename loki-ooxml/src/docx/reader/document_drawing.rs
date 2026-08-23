// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:drawing` parsing, split out of `document.rs` for the 300-line ceiling:
//! `parse_drawing` and its `wrapText` helper. `parse_drawing` is re-exported
//! `pub(crate)` from the parent (called by the `run` submodule).

use quick_xml::{Reader, events::Event};

use loki_doc_model::content::float::{FloatAlign, FloatWrap, TextWrap, WrapDistance, WrapSide};

use crate::docx::model::paragraph::DocxDrawing;
use crate::docx::reader::util::{attr_val, local_name, parse_emu};
use crate::error::{OoxmlError, OoxmlResult};

/// Parses a `w:drawing` element. Called after Start("drawing") is consumed.
pub(crate) fn parse_drawing(reader: &mut Reader<&[u8]>) -> OoxmlResult<DocxDrawing> {
    let mut drawing = DocxDrawing::default();
    // Wrap mode/side are carried on a `wp:wrap*` child; `behindDoc` lives on the
    // `wp:anchor` element. Collect both, then assemble the `FloatWrap` at the end.
    let mut wrap_mode: Option<TextWrap> = None;
    let mut wrap_side = WrapSide::Both;
    let mut behind_doc = false;
    // `wp:positionH` holds the object's own horizontal placement as the *text*
    // of a nested `wp:align`, so the value arrives on the following Text event.
    let mut align: Option<FloatAlign> = None;
    let mut dist: Option<WrapDistance> = None;
    let mut in_position_h = false;
    // `true` while inside an `a:ln` (border) element, so its `a:srgbClr` is read
    // as the border colour rather than the shape fill.
    let mut in_ln = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"anchor" => {
                        drawing.is_anchor = true;
                        behind_doc = attr_val(e, b"behindDoc").as_deref() == Some("1");
                        // Wrap clearance (EMU). Present iff the producer stated
                        // any of the four; absence is not zero — see
                        // `FloatWrap::dist`.
                        let emu =
                            |name: &[u8]| attr_val(e, name).and_then(|v| v.parse::<i64>().ok());
                        let sides = [emu(b"distT"), emu(b"distB"), emu(b"distL"), emu(b"distR")];
                        if sides.iter().any(Option::is_some) {
                            dist = Some(WrapDistance {
                                top: sides[0].unwrap_or(0),
                                bottom: sides[1].unwrap_or(0),
                                left: sides[2].unwrap_or(0),
                                right: sides[3].unwrap_or(0),
                            });
                        }
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
                    // `wps` text box: `a:ln` border, `a:srgbClr` fill/border, and
                    // `w:txbxContent` block content.
                    b"ln" => {
                        in_ln = true;
                        drawing.line_w_emu = attr_val(e, b"w").as_deref().and_then(parse_emu);
                    }
                    b"srgbClr" => {
                        if in_ln {
                            drawing.line_color = attr_val(e, b"val");
                        } else if drawing.fill_color.is_none() {
                            drawing.fill_color = attr_val(e, b"val");
                        }
                    }
                    b"txbxContent" => parse_txbx_content(reader, &mut drawing)?,
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
                    b"positionH" => in_position_h = true,
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) if in_position_h => {
                // Only `wp:align` carries a keyword; a `wp:posOffset` is an EMU
                // number, which this leaves unstated rather than guessing a
                // side from — see `TODO(float-pos-offset)` below.
                if let Ok(v) = crate::xml_util::event_text(&Event::Text(t.clone()))
                    && let Some(a) = FloatAlign::from_str_kw(v.trim())
                {
                    align = Some(a);
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"positionH" => {
                in_position_h = false;
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"ln" => {
                in_ln = false;
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
        // TODO(float-pos-offset): `wp:positionH/wp:posOffset` (an absolute EMU
        // offset instead of a keyword) is left unstated, so such a float falls
        // back to inferring its side from `wrapText`. Honouring it needs an
        // offset in the model, not just a three-way alignment.
        drawing.wrap = Some(FloatWrap {
            wrap,
            side: wrap_side,
            align,
            behind_text: behind_doc,
            dist,
        });
    }
    Ok(drawing)
}

/// Parse a `w:txbxContent` body (a text box's block content) into the drawing's
/// paragraph list. Called after its Start event; consumes through the matching
/// End. Reuses the top-level paragraph reader so runs/formatting are preserved.
fn parse_txbx_content(reader: &mut Reader<&[u8]>, drawing: &mut DocxDrawing) -> OoxmlResult<()> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"p" => {
                drawing
                    .txbx
                    .push(crate::docx::reader::document::parse_paragraph(reader)?);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"txbxContent" => {
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

/// Reads the `wrapText` attribute of a `wp:wrap*` element into a [`WrapSide`].
fn parse_wrap_text(e: &quick_xml::events::BytesStart<'_>) -> WrapSide {
    match attr_val(e, b"wrapText").as_deref() {
        Some("left") => WrapSide::Left,
        Some("right") => WrapSide::Right,
        Some("largest") => WrapSide::Largest,
        _ => WrapSide::Both,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> DocxDrawing {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Start(ref e) if local_name(e.local_name().as_ref()) == b"drawing" => break,
                Event::Eof => panic!("no drawing"),
                _ => {}
            }
        }
        parse_drawing(&mut reader).unwrap()
    }

    /// A float's own horizontal placement (`wp:positionH`) is read, and is
    /// distinct from the side text may wrap on (`@wrapText`).
    ///
    /// Nothing covered the import of `wp:positionH`, so the reader could stop
    /// reading it and only a golden would notice — a mutation that never set
    /// it survived the layout-side test.
    #[test]
    fn reads_the_float_position_separately_from_the_wrap_side() {
        let drawing = |position_h: &str| {
            let xml = format!(
                r#"<w:drawing xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
                  <wp:anchor behindDoc="0">{position_h}<wp:extent cx="914400" cy="914400"/>
                    <wp:wrapSquare wrapText="bothSides"/>
                  </wp:anchor></w:drawing>"#
            );
            parse(&xml).wrap.expect("a wrap is recorded")
        };

        // The `acid2-docx` newsletter figure's shape: `bothSides` wrap with an
        // explicit right placement. The two must not be conflated — the wrap
        // side stays `Both` while the placement is read as `Right`.
        let right = drawing(
            "<wp:positionH relativeFrom=\"column\"><wp:align>right</wp:align></wp:positionH>",
        );
        assert_eq!(right.align, Some(FloatAlign::Right));
        assert_eq!(right.side, WrapSide::Both, "the wrap side is unchanged");

        let left = drawing(
            "<wp:positionH relativeFrom=\"column\"><wp:align>left</wp:align></wp:positionH>",
        );
        assert_eq!(left.align, Some(FloatAlign::Left));

        // Absent, and the un-keyworded `posOffset` form, are both *unstated* —
        // the layout then falls back to inferring from the wrap side rather
        // than guessing a placement. Without these two the test would pass on a
        // reader that answered `Right` for everything.
        assert_eq!(drawing("").align, None, "no `wp:positionH` states nothing");
        assert_eq!(
            drawing("<wp:positionH relativeFrom=\"column\"><wp:posOffset>457200</wp:posOffset></wp:positionH>").align,
            None,
            "a `wp:posOffset` is an EMU number, not a placement keyword"
        );
    }

    #[test]
    fn parses_wps_text_box() {
        let xml = r#"<w:drawing xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
          <wp:anchor behindDoc="0"><wp:extent cx="1828800" cy="731520"/><wp:wrapSquare wrapText="left"/>
            <a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
              <wps:wsp><wps:spPr>
                <a:solidFill><a:srgbClr val="FDF0E6"/></a:solidFill>
                <a:ln w="12700"><a:solidFill><a:srgbClr val="ED7D31"/></a:solidFill></a:ln>
              </wps:spPr>
              <wps:txbx><w:txbxContent>
                <w:p><w:r><w:t>Box text.</w:t></w:r></w:p>
              </w:txbxContent></wps:txbx></wps:wsp>
            </a:graphicData></a:graphic>
          </wp:anchor></w:drawing>"#;
        let d = parse(xml);
        assert_eq!(d.txbx.len(), 1, "one text-box paragraph");
        assert_eq!(d.fill_color.as_deref(), Some("FDF0E6"));
        assert_eq!(d.line_color.as_deref(), Some("ED7D31"));
        assert!(d.is_anchor);
        assert_eq!(d.cx, Some(1_828_800));
    }
}
