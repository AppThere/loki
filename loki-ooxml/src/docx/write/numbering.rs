// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Numbering state collector and `word/numbering.xml` serializer.
//!
//! Lists in the doc model (`Block::BulletList`, `Block::OrderedList`) are
//! serialized as OOXML paragraph `w:numPr` references backed by entries in
//! `word/numbering.xml`.  This module tracks which lists were encountered
//! during document serialization and produces the corresponding XML.
//!
//! ECMA-376 §17.9 (Numbering Definitions).

use quick_xml::Writer;

use crate::docx::write::xml::{
    write_decl, write_empty, write_end, write_start, write_text_elem, wval, NS_W,
};

use loki_doc_model::content::block::{ListAttributes, ListNumberStyle};

/// The kind of list represented by a `w:abstractNum`.
#[derive(Debug, Clone)]
enum AbstractKind {
    Bullet,
    Ordered { start: u32, num_fmt: &'static str },
}

struct AbstractNum {
    id: u32,
    kind: AbstractKind,
}

struct Num {
    num_id: u32,
    abstract_num_id: u32,
}

/// Accumulated list state built up while serializing `word/document.xml`.
///
/// Each unique list encountered registers itself here and receives a `numId`
/// to embed in paragraph `w:numPr` elements. After document serialization,
/// call [`write_numbering_xml`] to produce `word/numbering.xml`.
pub(super) struct NumberingState {
    /// abstractNum for bullet lists (shared among all bullet lists).
    bullet_abstract_id: Option<u32>,
    /// All abstract nums in definition order.
    abstracts: Vec<AbstractNum>,
    /// All concrete nums (numId → abstractNumId).
    nums: Vec<Num>,
    next_abstract_id: u32,
    next_num_id: u32,
}

impl NumberingState {
    pub(super) fn new() -> Self {
        Self {
            bullet_abstract_id: None,
            abstracts: Vec::new(),
            nums: Vec::new(),
            next_abstract_id: 0,
            next_num_id: 1,
        }
    }

    /// Register a bullet list.  Returns the `numId` to use for all
    /// paragraphs in this list.  All bullet lists share one `abstractNum`.
    pub(super) fn register_bullet(&mut self) -> u32 {
        let abstract_id = match self.bullet_abstract_id {
            Some(id) => id,
            None => {
                let id = self.next_abstract_id;
                self.next_abstract_id += 1;
                self.abstracts.push(AbstractNum {
                    id,
                    kind: AbstractKind::Bullet,
                });
                self.bullet_abstract_id = Some(id);
                id
            }
        };
        let num_id = self.next_num_id;
        self.next_num_id += 1;
        self.nums.push(Num { num_id, abstract_num_id: abstract_id });
        num_id
    }

    /// Register an ordered list.  Returns the `numId` to use for this list.
    /// Each ordered list gets its own `abstractNum` so start values are correct.
    pub(super) fn register_ordered(&mut self, attrs: &ListAttributes) -> u32 {
        let num_fmt = match attrs.style {
            ListNumberStyle::LowerAlpha => "lowerLetter",
            ListNumberStyle::UpperAlpha => "upperLetter",
            ListNumberStyle::LowerRoman => "lowerRoman",
            ListNumberStyle::UpperRoman => "upperRoman",
            _ => "decimal",
        };
        let start = attrs.start_number.max(1) as u32;
        let abstract_id = self.next_abstract_id;
        self.next_abstract_id += 1;
        self.abstracts.push(AbstractNum {
            id: abstract_id,
            kind: AbstractKind::Ordered { start, num_fmt },
        });
        let num_id = self.next_num_id;
        self.next_num_id += 1;
        self.nums.push(Num { num_id, abstract_num_id: abstract_id });
        num_id
    }

    /// Returns `true` if no lists were registered (no `numbering.xml` needed).
    pub(super) fn is_empty(&self) -> bool {
        self.abstracts.is_empty()
    }
}

/// Serializes all registered numbering definitions to `word/numbering.xml` bytes.
pub(super) fn write_numbering_xml(state: &NumberingState) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    let _ = write_decl(&mut w);

    let _ = write_start(
        &mut w,
        "w:numbering",
        &[("xmlns:w", NS_W)],
    );

    for abs in &state.abstracts {
        let abs_id_s = abs.id.to_string();
        let _ = write_start(&mut w, "w:abstractNum", &[("w:abstractNumId", &abs_id_s)]);

        match &abs.kind {
            AbstractKind::Bullet => {
                let _ = write_empty(&mut w, "w:multiLevelType", &wval("hybridMultilevel"));
                let _ = write_start(&mut w, "w:lvl", &[("w:ilvl", "0")]);
                let _ = write_empty(&mut w, "w:start", &wval("1"));
                let _ = write_empty(&mut w, "w:numFmt", &wval("bullet"));
                let _ = write_text_elem(&mut w, "w:lvlText", &wval("\u{2022}"), "");
                let _ = write_empty(&mut w, "w:lvlJc", &wval("left"));
                let _ = write_start(&mut w, "w:pPr", &[]);
                let _ = write_empty(
                    &mut w,
                    "w:ind",
                    &[("w:left", "720"), ("w:hanging", "360")],
                );
                let _ = write_end(&mut w, "w:pPr");
                let _ = write_end(&mut w, "w:lvl");
            }
            AbstractKind::Ordered { start, num_fmt } => {
                let _ = write_empty(&mut w, "w:multiLevelType", &wval("singleLevel"));
                let _ = write_start(&mut w, "w:lvl", &[("w:ilvl", "0")]);
                let start_s = start.to_string();
                let _ = write_empty(&mut w, "w:start", &wval(&start_s));
                let _ = write_empty(&mut w, "w:numFmt", &wval(num_fmt));
                let _ = write_text_elem(&mut w, "w:lvlText", &wval("%1."), "");
                let _ = write_empty(&mut w, "w:lvlJc", &wval("left"));
                let _ = write_start(&mut w, "w:pPr", &[]);
                let _ = write_empty(
                    &mut w,
                    "w:ind",
                    &[("w:left", "720"), ("w:hanging", "360")],
                );
                let _ = write_end(&mut w, "w:pPr");
                let _ = write_end(&mut w, "w:lvl");
            }
        }

        let _ = write_end(&mut w, "w:abstractNum");
    }

    for num in &state.nums {
        let num_id_s = num.num_id.to_string();
        let _ = write_start(&mut w, "w:num", &[("w:numId", &num_id_s)]);
        let abs_id_s = num.abstract_num_id.to_string();
        let _ = write_empty(&mut w, "w:abstractNumId", &wval(&abs_id_s));
        let _ = write_end(&mut w, "w:num");
    }

    let _ = write_end(&mut w, "w:numbering");
    drop(w);
    out
}

