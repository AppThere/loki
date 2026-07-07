// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for the `text:tracked-changes` region table. ODF 1.3 §5.5.3.
//!
//! Each `<text:changed-region text:id="…">` holds a `<text:insertion>` or
//! `<text:deletion>` wrapper with an `<office:change-info>` (`dc:creator` /
//! `dc:date`); a deletion also carries the removed `<text:p>` content. The
//! parsed regions are matched against the body `text:change-*` milestones during
//! mapping. This mirrors the comment reader in [`super::annotations`].

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::revision::{OdfChangeKind, OdfChangedRegion};
use crate::xml_util::local_attr_val;

/// Parses a `text:tracked-changes` element (its `Start` already consumed) into
/// the list of changed regions it contains.
pub(crate) fn read_tracked_changes(reader: &mut Reader<&[u8]>) -> OdfResult<Vec<OdfChangedRegion>> {
    let mut regions = Vec::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.local_name().into_inner() == b"changed-region" {
                    let id = local_attr_val(e, b"id").unwrap_or_default();
                    regions.push(read_changed_region(reader, id)?);
                }
            }
            Ok(Event::End(ref e)) if e.local_name().into_inner() == b"tracked-changes" => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
    }
    Ok(regions)
}

/// Reads one `text:changed-region` (its `Start` already consumed), collecting
/// the change kind, author/date, and any deleted text.
fn read_changed_region(reader: &mut Reader<&[u8]>, id: String) -> OdfResult<OdfChangedRegion> {
    let mut kind = OdfChangeKind::Insertion;
    let mut creator = None;
    let mut date = None;
    let mut deleted: Vec<String> = Vec::new();

    let mut buf = Vec::new();
    // Which leaf element's text we are collecting (`creator` / `date` / `p`).
    let mut collecting: Option<&'static str> = None;
    let mut text = String::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.local_name().into_inner() {
                b"insertion" => kind = OdfChangeKind::Insertion,
                b"deletion" => kind = OdfChangeKind::Deletion,
                b"creator" => collecting = Some("creator"),
                b"date" => collecting = Some("date"),
                b"p" => collecting = Some("p"),
                _ => {}
            },
            Ok(Event::Text(ref t)) => {
                if collecting.is_some() {
                    text.push_str(&t.unescape().map_err(xml_err)?);
                }
            }
            Ok(Event::End(ref e)) => match e.local_name().into_inner() {
                b"creator" => {
                    creator = Some(std::mem::take(&mut text));
                    collecting = None;
                }
                b"date" => {
                    date = Some(std::mem::take(&mut text));
                    collecting = None;
                }
                b"p" => {
                    deleted.push(std::mem::take(&mut text));
                    collecting = None;
                }
                b"changed-region" => break,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
    }

    Ok(OdfChangedRegion {
        change_id: id,
        kind,
        creator,
        date,
        deleted_text: deleted.join("\n"),
    })
}

fn xml_err(source: quick_xml::Error) -> OdfError {
    OdfError::Xml {
        part: "content.xml".to_string(),
        source,
    }
}
