// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Builds `word/_rels/document.xml.rels` — the relationships from the main
//! document part to its styles, numbering, notes, hyperlinks, media,
//! headers/footers, and settings parts.

use loki_opc::Package;
use loki_opc::part::PartName;
use loki_opc::relationships::{Relationship, TargetMode};

use crate::docx::write::collector::{ExportCollector, HeaderFooterEntry};
use crate::docx::write::settings::wire_settings;
use crate::docx::write::xml::{
    REL_ENDNOTES, REL_FOOTER, REL_FOOTNOTES, REL_HEADER, REL_HYPERLINK, REL_IMAGE, REL_NUMBERING,
    REL_STYLES,
};
use crate::error::OoxmlError;

/// Adds one internal/external relationship to `word/_rels/document.xml.rels`.
fn add(
    pkg: &mut Package,
    doc_part: &PartName,
    id: &str,
    rel_type: &str,
    target: &str,
    mode: TargetMode,
) -> Result<(), OoxmlError> {
    pkg.part_relationships_mut(doc_part)
        .add(Relationship {
            id: id.to_string(),
            rel_type: rel_type.to_string(),
            target: target.to_string(),
            target_mode: mode,
        })
        .map_err(OoxmlError::Opc)
}

/// Wires every document-level relationship. Fixed-rId parts (styles rId1;
/// numbering/footnotes/endnotes rId2-4) come first; collector-assigned rIds
/// (hyperlinks, media, headers/footers, settings) follow.
pub(super) fn add_document_relationships(
    pkg: &mut Package,
    doc_part: &PartName,
    collector: &mut ExportCollector,
    headers_footers: &[HeaderFooterEntry],
    has: AuxParts,
) -> Result<(), OoxmlError> {
    use TargetMode::{External, Internal};

    add(pkg, doc_part, "rId1", REL_STYLES, "styles.xml", Internal)?;
    if has.numbering {
        add(
            pkg,
            doc_part,
            "rId2",
            REL_NUMBERING,
            "numbering.xml",
            Internal,
        )?;
    }
    if has.footnotes {
        add(
            pkg,
            doc_part,
            "rId3",
            REL_FOOTNOTES,
            "footnotes.xml",
            Internal,
        )?;
    }
    if has.endnotes {
        add(
            pkg,
            doc_part,
            "rId4",
            REL_ENDNOTES,
            "endnotes.xml",
            Internal,
        )?;
    }

    for (r_id, url) in &collector.hyperlinks {
        add(pkg, doc_part, r_id, REL_HYPERLINK, url, External)?;
    }
    for m in &collector.media {
        let target = m.path.strip_prefix("word/").unwrap_or(&m.path);
        add(pkg, doc_part, &m.r_id, REL_IMAGE, target, Internal)?;
    }
    for hf in headers_footers {
        let rel_type = if hf.is_header { REL_HEADER } else { REL_FOOTER };
        let target = hf.path.strip_prefix("word/").unwrap_or(&hf.path);
        add(pkg, doc_part, &hf.r_id, rel_type, target, Internal)?;
    }

    // Settings (reserve an rId now that all H/F + media relationships exist).
    if has.even_odd {
        let r_id = collector.reserve_r_id();
        wire_settings(pkg, doc_part, r_id)?;
    }
    Ok(())
}

/// Flags for which optional auxiliary parts/relationships are present.
///
// A plain presence-flag carrier, not a state machine — the lint's
// "two-variant enum" suggestion does not apply.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub(super) struct AuxParts {
    pub numbering: bool,
    pub footnotes: bool,
    pub endnotes: bool,
    pub even_odd: bool,
}
