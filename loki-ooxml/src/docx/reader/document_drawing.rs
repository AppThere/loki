// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:drawing` parsing, split out of `document.rs` for the 300-line ceiling:
//! `parse_drawing` and its `wrapText` helper. `parse_drawing` is re-exported
//! `pub(crate)` from the parent (called by the `run` submodule).

use quick_xml::{Reader, events::Event};

use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

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
