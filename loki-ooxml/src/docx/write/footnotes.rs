// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Footnotes and endnotes serialization.

use quick_xml::Writer;

use loki_doc_model::content::inline::NoteKind;

use super::collector::{ExportCollector, FootnoteEntry};
use super::document::write_blocks;
use super::xml::{NS_W, write_decl, write_empty, write_end, write_start, wval};

pub fn write_footnotes_xml(collector: &mut ExportCollector) -> Vec<u8> {
    let entries = collector.take_footnotes();
    write_notes_xml(entries, NoteKind::Footnote, collector)
}

pub fn write_endnotes_xml(collector: &mut ExportCollector) -> Vec<u8> {
    let entries = collector.take_endnotes();
    write_notes_xml(entries, NoteKind::Endnote, collector)
}

fn write_notes_xml(
    entries: Vec<FootnoteEntry>,
    kind: NoteKind,
    collector: &mut ExportCollector,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new_with_indent(&mut out, b' ', 2);

    let _ = write_decl(&mut w);

    let root = match kind {
        NoteKind::Footnote => "w:footnotes",
        NoteKind::Endnote => "w:endnotes",
        _ => return Vec::new(),
    };
    let _ = write_start(&mut w, root, &[("xmlns:w", NS_W)]);

    // Magic separator notes.
    write_special_note(&mut w, "separator", "-1", "w:separator");
    write_special_note(
        &mut w,
        "continuationSeparator",
        "0",
        "w:continuationSeparator",
    );

    for entry in entries {
        let tag = match kind {
            NoteKind::Footnote => "w:footnote",
            NoteKind::Endnote => "w:endnote",
            _ => continue,
        };
        let _ = write_start(&mut w, tag, &[("w:id", &entry.id.to_string())]);

        // Footnotes/Endnotes are a collection of blocks.
        // Usually the first block is a paragraph that needs the reference.
        // To keep it simple, we'll emit a paragraph with the reference,
        // then emit the entry's blocks.

        let _ = write_start(&mut w, "w:p", &[]);
        let _ = write_start(&mut w, "w:pPr", &[]);
        let style = match kind {
            NoteKind::Footnote => "FootnoteText",
            NoteKind::Endnote => "EndnoteText",
            _ => "Normal",
        };
        let _ = write_empty(&mut w, "w:pStyle", &wval(style));
        let _ = write_end(&mut w, "w:pPr");

        let _ = write_start(&mut w, "w:r", &[]);
        let _ = write_start(&mut w, "w:rPr", &[]);
        let ref_style = match kind {
            NoteKind::Footnote => "FootnoteReference",
            NoteKind::Endnote => "EndnoteReference",
            _ => "DefaultParagraphFont",
        };
        let _ = write_empty(&mut w, "w:rStyle", &wval(ref_style));
        let _ = write_end(&mut w, "w:rPr");

        let ref_elem = match kind {
            NoteKind::Footnote => "w:footnoteRef",
            NoteKind::Endnote => "w:endnoteRef",
            _ => continue,
        };
        let _ = write_empty(&mut w, ref_elem, &[]);
        let _ = write_end(&mut w, "w:r");

        // Space after reference.
        let _ = write_start(&mut w, "w:r", &[]);
        let _ = crate::docx::write::xml::write_text_elem(
            &mut w,
            "w:t",
            &[("xml:space", "preserve")],
            " ",
        );
        let _ = write_end(&mut w, "w:r");

        // If the first block is a paragraph, we might want to merge it,
        // but for now we'll just emit it as a separate paragraph for simplicity,
        // although it's not perfectly standard (Word prefers the first line of the note
        // to be the same paragraph as the reference).
        // Actually, let's just close this paragraph and let write_blocks do its thing.
        let _ = write_end(&mut w, "w:p");

        write_blocks(&mut w, &entry.blocks, collector, 0);

        let _ = write_end(&mut w, tag);
    }

    let _ = write_end(&mut w, root);
    out
}

fn write_special_note<W: std::io::Write>(
    w: &mut Writer<W>,
    type_val: &str,
    id_val: &str,
    child: &str,
) {
    let tag = if child.contains("footnote")
        || child == "w:separator"
        || child == "w:continuationSeparator"
    {
        "w:footnote"
    } else {
        "w:endnote"
    };
    // Actually, word uses w:footnote for both in footnotes.xml and w:endnote in endnotes.xml.
    // The child determines the type if it's separator.

    let _ = write_start(w, tag, &[("w:type", type_val), ("w:id", id_val)]);
    let _ = write_start(w, "w:p", &[]);
    let _ = write_start(w, "w:r", &[]);
    let _ = write_empty(w, child, &[]);
    let _ = write_end(w, "w:r");
    let _ = write_end(w, "w:p");
    let _ = write_end(w, tag);
}
