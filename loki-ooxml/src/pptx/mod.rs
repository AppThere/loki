// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PPTX (`PresentationML`) import.
//!
//! Maps an OOXML presentation package to the format-neutral
//! [`loki_presentation_model::Presentation`] (a deck of slides over
//! [`loki_graphics`]). This is an MVP importer: it reads the slide size, the
//! ordered slide list, and per-slide shapes (`p:sp`) and pictures (`p:pic`) —
//! their transform, preset geometry, solid fill, line stroke, text, and
//! placeholder role. Groups (`p:grpSp`), tables/charts (`p:graphicFrame`),
//! connectors, custom geometry, gradients, and theme-color / layout-inherited
//! properties are not yet resolved and are reported as warnings.

pub mod export;
pub mod import;
mod presentation_part;
mod shapes;
mod sppr;
mod text;
mod units;
mod write_presentation;
mod write_slide;

use loki_graphics::Shape;
use loki_presentation_model::PlaceholderKind;
use quick_xml::Reader;
use quick_xml::events::{BytesEnd, Event};
use std::io::BufRead;

/// Whether an end tag's local name (namespace prefix stripped) equals `name`.
pub(super) fn end_is(e: &BytesEnd<'_>, name: &[u8]) -> bool {
    e.name().local_name().into_inner() == name
}

/// Escapes a string for inclusion in XML text or a double-quoted attribute.
pub(super) fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A shape parsed from a slide, plus its placeholder role (if any).
pub(super) struct ParsedShape {
    /// The graphic shape.
    pub shape: Shape,
    /// Placeholder role, when the shape is a layout placeholder.
    pub placeholder: Option<PlaceholderKind>,
}

/// Consumes events until the currently-open element's matching end tag.
///
/// Assumes the opening `Start` was already read. Uses its own buffer so callers
/// can keep borrowing the event they matched on.
pub(super) fn skip_subtree<R: BufRead>(reader: &mut Reader<R>) -> Result<(), quick_xml::Error> {
    let mut depth = 1usize;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
        buf.clear();
    }
}

/// Reads the first `a:srgbClr@val` inside an already-opened `a:solidFill` and
/// returns its color, consuming up to the `solidFill` end tag.
pub(super) fn parse_solid_fill_color<R: BufRead>(
    reader: &mut Reader<R>,
) -> Result<Option<loki_primitives::color::DocumentColor>, quick_xml::Error> {
    use crate::xml_util::{local_attr_val, local_name};

    let mut color = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                if local_name(&e) == b"srgbClr" {
                    if let Some(hex) = local_attr_val(&e, b"val") {
                        color = units::color_from_srgb(&hex);
                    }
                }
            }
            Event::End(e) if end_is(&e, b"solidFill") => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(color)
}
