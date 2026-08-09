// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for the advisory page-style part (Spec 08 T6.5, D-02).
//!
//! Everything this module returns is *advice*. It is parsed after the document
//! itself is built, checked against that document, and dropped whole if it does
//! not fit — see [`crate::docx::page_style_part`] for why the check is against
//! the section count and why a partial application is worse than none.

use loki_doc_model::document::Document;
use loki_opc::{Package, PartName};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::page_style_part::{
    PageStyleMap, PartStyle, REL_PAGE_STYLES, apply_page_style_part,
};
use crate::docx::reader::util::{attr_val, local_name};

/// Finds, parses and applies the advisory part, if the package has one.
///
/// Returns whether names were applied — `false` covers "no such part" and
/// "present but did not describe this document" alike, because neither is an
/// error and both leave the importer's own naming in place.
pub(crate) fn apply_advisory_page_styles(package: &Package, doc: &mut Document) -> bool {
    let Some(map) = read_page_style_part(package) else {
        return false;
    };
    apply_page_style_part(doc, &map)
}

/// Locates the part through the **package-level** relationships and parses it.
///
/// `None` when absent or malformed: a part we cannot read is not a document we
/// should refuse to open, since by construction nothing in it affects content.
fn read_page_style_part(package: &Package) -> Option<PageStyleMap> {
    let part_name = PartName::new(crate::docx::page_style_part::PAGE_STYLE_PART).ok()?;
    // The relationship is what makes the part ours; the fixed name is a
    // convenience. A package that renamed the part but kept the relationship is
    // still readable, and one that has the name but not the relationship is not
    // treated as ours.
    let related = package
        .relationships()
        .iter()
        .any(|r| r.rel_type == REL_PAGE_STYLES);
    if !related {
        return None;
    }
    let part = package.part(&part_name)?;
    parse_page_style_part(&part.bytes)
}

/// Parses the part's XML. `None` on a malformed document or a missing
/// `sectionCount`.
///
/// The declared `sectionCount` is authoritative for the map's length, and the
/// `<section>` elements fill it in by index. A file listing three sections but
/// claiming four therefore produces a four-long map with a hole — which fails
/// the count check against a three-section document, rather than silently
/// looking like a three-section map.
fn parse_page_style_part(xml: &[u8]) -> Option<PageStyleMap> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut styles: Vec<PartStyle> = Vec::new();
    let mut entries: Vec<(usize, String)> = Vec::new();
    let mut declared_count: Option<usize> = None;

    loop {
        let event = reader.read_event_into(&mut buf).ok()?;
        match event {
            // `local_name(e.name().as_ref())`, not `local_name(e)`: `BytesStart`
            // derefs to the element's *whole* raw bytes, attributes included, so
            // the shorter spelling compiles and then matches nothing.
            Event::Start(ref e) | Event::Empty(ref e) => match local_name(e.name().as_ref()) {
                b"pageStyles" => {
                    declared_count = attr_val(e, b"sectionCount")?.parse().ok();
                }
                b"style" => {
                    let id = attr_val(e, b"id")?;
                    if id.is_empty() {
                        return None;
                    }
                    styles.push(PartStyle {
                        id,
                        display_name: attr_val(e, b"displayName").filter(|n| !n.is_empty()),
                    });
                }
                b"section" => {
                    let index: usize = attr_val(e, b"index")?.parse().ok()?;
                    let style = attr_val(e, b"style")?;
                    entries.push((index, style));
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let count = declared_count?;
    // A count large enough to be a denial-of-service rather than a document.
    // The check against the real section count would catch it anyway; this
    // stops the allocation that would happen first.
    if count > 100_000 {
        return None;
    }
    let mut sections = vec![None; count];
    for (index, style) in entries {
        // Out of range, or two entries for one section: either means the part
        // does not describe a document we can trust it about.
        if index >= count || sections[index].is_some() {
            return None;
        }
        sections[index] = Some(style);
    }
    Some(PageStyleMap { styles, sections })
}

#[cfg(test)]
#[path = "page_style_part_tests.rs"]
mod tests;
