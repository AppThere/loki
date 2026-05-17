// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Export state collector for hyperlinks, media, and footnotes.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;

use crate::docx::write::media::extract_data_uri;
use crate::docx::write::numbering::NumberingState;

/// Tracks image data to be embedded in the DOCX package.
pub struct MediaEntry {
    pub r_id: String,
    pub path: String, // e.g. "word/media/image1.png"
    pub bytes: Vec<u8>,
    pub ext: String,
}

/// Tracks footnote content to be written to `word/footnotes.xml`.
pub struct FootnoteEntry {
    pub id: u32,
    pub blocks: Vec<Block>,
}

/// Tracks header or footer content to be written to its own part.
pub struct HeaderFooterEntry {
    pub r_id: String,
    pub path: String, // e.g. "word/header1.xml"
    pub blocks: Vec<Block>,
    pub is_header: bool,
}

/// Accumulated state built up while serializing `word/document.xml`.
pub struct ExportCollector {
    /// List numbering state.
    pub num_state: NumberingState,
    /// Collected hyperlinks: (rId, url).
    pub hyperlinks: Vec<(String, String)>,
    /// Collected media entries.
    pub media: Vec<MediaEntry>,
    /// Map of data URI src to assigned rId to avoid duplicates.
    media_by_src: HashMap<String, String>,
    /// Collected footnotes.
    pub footnotes: Vec<FootnoteEntry>,
    /// Collected endnotes.
    pub endnotes: Vec<FootnoteEntry>,
    /// Collected headers and footers.
    pub headers_footers: Vec<HeaderFooterEntry>,

    next_r_id: u32,
    next_footnote_id: u32,
    next_endnote_id: u32,
}

impl ExportCollector {
    pub fn new() -> Self {
        Self {
            num_state: NumberingState::new(),
            hyperlinks: Vec::new(),
            media: Vec::new(),
            media_by_src: HashMap::new(),
            footnotes: Vec::new(),
            endnotes: Vec::new(),
            headers_footers: Vec::new(),
            // word/_rels/document.xml.rels usually starts from rId1.
            // Styles is rId1, Numbering is rId2 (if present).
            // We'll start from rId10 to be safe and clear.
            next_r_id: 10,
            next_footnote_id: 1,
            next_endnote_id: 1,
        }
    }

    /// Registers a header or footer and returns its assigned rId.
    pub fn add_header_footer(&mut self, blocks: Vec<Block>, is_header: bool) -> String {
        let n = self.headers_footers.len() + 1;
        let r_id = format!("rId{}", self.next_r_id);
        self.next_r_id += 1;
        let prefix = if is_header { "header" } else { "footer" };
        let path = format!("word/{prefix}{n}.xml");
        self.headers_footers.push(HeaderFooterEntry {
            r_id: r_id.clone(),
            path,
            blocks,
            is_header,
        });
        r_id
    }

    /// Registers a hyperlink and returns its assigned rId.
    pub fn add_hyperlink(&mut self, url: &str) -> String {
        let r_id = format!("rId{}", self.next_r_id);
        self.next_r_id += 1;
        self.hyperlinks.push((r_id.clone(), url.to_string()));
        r_id
    }

    /// Registers an image and returns its assigned rId.
    pub fn add_image(&mut self, src: &str) -> Option<String> {
        if let Some(r_id) = self.media_by_src.get(src) {
            return Some(r_id.clone());
        }
        let (ext, bytes) = extract_data_uri(src)?;
        let n = self.media.len() + 1;
        let r_id = format!("rId{}", self.next_r_id);
        self.next_r_id += 1;
        let path = format!("word/media/image{n}.{ext}");
        self.media_by_src.insert(src.to_string(), r_id.clone());
        self.media.push(MediaEntry {
            r_id: r_id.clone(),
            path,
            bytes,
            ext,
        });
        Some(r_id)
    }

    /// Registers a footnote and returns its numeric ID.
    pub fn add_footnote(&mut self, blocks: Vec<Block>) -> u32 {
        let id = self.next_footnote_id;
        self.next_footnote_id += 1;
        self.footnotes.push(FootnoteEntry { id, blocks });
        id
    }

    /// Registers an endnote and returns its numeric ID.
    pub fn add_endnote(&mut self, blocks: Vec<Block>) -> u32 {
        let id = self.next_endnote_id;
        self.next_endnote_id += 1;
        self.endnotes.push(FootnoteEntry { id, blocks });
        id
    }

    pub fn take_footnotes(&mut self) -> Vec<FootnoteEntry> {
        std::mem::take(&mut self.footnotes)
    }

    pub fn take_endnotes(&mut self) -> Vec<FootnoteEntry> {
        std::mem::take(&mut self.endnotes)
    }

    pub fn take_headers_footers(&mut self) -> Vec<HeaderFooterEntry> {
        std::mem::take(&mut self.headers_footers)
    }
}
