// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Helpers that collect a sequence of `w:r` runs nested inside a wrapper
//! element (`w:hyperlink`, `w:del`/`w:ins`, `w:fldSimple`).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::docx::model::paragraph::{
    DocxMarkRevision, DocxRevisionInfo, DocxRun, DocxTrackedChange,
};
use crate::docx::reader::document::parse_run;
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

/// Consumes runs inside a `w:hyperlink` element.
pub(crate) fn parse_hyperlink_runs(reader: &mut Reader<&[u8]>) -> OoxmlResult<Vec<DocxRun>> {
    collect_runs(reader, b"hyperlink")
}

/// Consumes a `w:del` / `w:ins` tracked change: its `w:author`/`w:date`/`w:id`
/// attributes (from `start`) plus the wrapped runs.
pub(crate) fn parse_tracked_runs(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    end_tag: &[u8],
) -> OoxmlResult<DocxTrackedChange> {
    let info = DocxRevisionInfo {
        author: attr_val(start, b"author"),
        date: attr_val(start, b"date"),
        id: attr_val(start, b"id"),
    };
    let runs = collect_runs(reader, end_tag)?;
    Ok(DocxTrackedChange { info, runs })
}

/// Reads a `w:del` / `w:ins` element that marks a **paragraph mark** deleted /
/// inserted (a child of `w:pPr/w:rPr`), from its (empty) element. `local` is the
/// element's local name (`b"del"` / `b"ins"`).
pub(crate) fn parse_mark_revision(local: &[u8], e: &BytesStart<'_>) -> DocxMarkRevision {
    DocxMarkRevision {
        is_deletion: local == b"del",
        info: DocxRevisionInfo {
            author: attr_val(e, b"author"),
            date: attr_val(e, b"date"),
            id: attr_val(e, b"id"),
        },
    }
}

/// Consumes the cached-result runs inside a `w:fldSimple` element.
///
/// A nested `w:fldSimple` (a field inside another field's result) has its own
/// result runs flattened into the outer field's result, which is sufficient for
/// recovering the displayed snapshot text.
pub(crate) fn parse_fld_simple_runs(reader: &mut Reader<&[u8]>) -> OoxmlResult<Vec<DocxRun>> {
    let mut runs = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"r" => runs.push(parse_run(reader)?),
                b"fldSimple" => runs.extend(parse_fld_simple_runs(reader)?),
                _ => {}
            },
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == b"fldSimple" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
        buf.clear();
    }
    Ok(runs)
}

/// Collects `w:r` runs until the matching `end_tag` end element is seen.
fn collect_runs(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> OoxmlResult<Vec<DocxRun>> {
    let mut runs = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.local_name().as_ref()) == b"r" => {
                runs.push(parse_run(reader)?);
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == end_tag => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(xml_err(e)),
            _ => {}
        }
        buf.clear();
    }
    Ok(runs)
}

fn xml_err(source: quick_xml::Error) -> OoxmlError {
    OoxmlError::Xml {
        part: "word/document.xml".into(),
        source,
    }
}
