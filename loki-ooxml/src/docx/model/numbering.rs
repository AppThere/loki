// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Intermediate model for `word/numbering.xml`.
//!
//! Mirrors ECMA-376 §17.9 (numbering definitions).
//! The three-level indirection: `w:numId → w:num → w:abstractNum`
//! is preserved in this model. See ADR-0005.

use super::paragraph::{DocxPPr, DocxRPr};

/// Top-level model for `w:numbering` (ECMA-376 §17.9.17).
#[derive(Debug, Clone, Default)]
pub struct DocxNumbering {
    /// Abstract numbering definitions (`w:abstractNum`).
    pub abstract_nums: Vec<DocxAbstractNum>,
    /// Numbering instances (`w:num`).
    pub nums: Vec<DocxNum>,
}

impl DocxNumbering {
    /// Resolves a `numId` to the corresponding `abstractNumId`.
    ///
    /// Returns `None` if the `numId` is not found or has no `abstractNumId`.
    #[must_use]
    pub fn abstract_num_id_for(&self, num_id: u32) -> Option<u32> {
        self.nums
            .iter()
            .find(|n| n.num_id == num_id)
            .map(|n| n.abstract_num_id)
    }

    /// Resolves a `numId` to its effective `DocxAbstractNum`, applying any
    /// level overrides from the `DocxNum`.
    ///
    /// Returns `None` if the id chain cannot be resolved.
    #[must_use]
    pub fn resolve(&self, num_id: u32) -> Option<ResolvedNumDef<'_>> {
        let num = self.nums.iter().find(|n| n.num_id == num_id)?;
        let abs = self
            .abstract_nums
            .iter()
            .find(|a| a.abstract_num_id == num.abstract_num_id)?;
        Some(ResolvedNumDef { abs, num })
    }
}

/// The result of resolving a numId to its abstract definition + instance overrides.
pub struct ResolvedNumDef<'a> {
    /// The base abstract numbering definition.
    pub abs: &'a DocxAbstractNum,
    /// The numbering instance (may contain level overrides).
    pub num: &'a DocxNum,
}

impl<'a> ResolvedNumDef<'a> {
    /// Returns the effective level definition at the given 0-indexed level,
    /// applying any override from `w:num/w:lvlOverride` if present.
    #[must_use]
    pub fn level(&self, ilvl: u8) -> Option<&DocxLevel> {
        // Check for override first
        if let Some(ov) = self.num.level_overrides.iter().find(|o| o.ilvl == ilvl) {
            if let Some(ref lvl) = ov.level {
                return Some(lvl);
            }
        }
        self.abs.levels.iter().find(|l| l.ilvl == ilvl)
    }
}

/// Abstract numbering definition (`w:abstractNum`, ECMA-376 §17.9.1).
#[derive(Debug, Clone)]
pub struct DocxAbstractNum {
    /// `@w:abstractNumId` — unique identifier.
    pub abstract_num_id: u32,
    /// Level definitions, one per indent level.
    pub levels: Vec<DocxLevel>,
}

/// Numbering instance (`w:num`, ECMA-376 §17.9.15).
#[derive(Debug, Clone)]
pub struct DocxNum {
    /// `@w:numId` — instance identifier referenced by paragraphs.
    pub num_id: u32,
    /// `w:abstractNumId @w:val` — references the abstract definition.
    pub abstract_num_id: u32,
    /// Level overrides (`w:lvlOverride`).
    pub level_overrides: Vec<DocxLvlOverride>,
}

/// A level override from `w:lvlOverride` (ECMA-376 §17.9.8).
#[derive(Debug, Clone)]
pub struct DocxLvlOverride {
    /// `@w:ilvl` — the level index being overridden.
    pub ilvl: u8,
    /// Optional start-value override.
    pub start_override: Option<u32>,
    /// Optional full level definition override.
    pub level: Option<DocxLevel>,
}

/// A list level definition from `w:lvl` (ECMA-376 §17.9.6).
#[derive(Debug, Clone)]
pub struct DocxLevel {
    /// `@w:ilvl` — zero-indexed level.
    pub ilvl: u8,
    /// `w:start @w:val` — starting number.
    pub start: Option<u32>,
    /// `w:numFmt @w:val` — numbering format string.
    pub num_fmt: Option<String>,
    /// `w:lvlText @w:val` — label format string (e.g. `"%1."`).
    pub lvl_text: Option<String>,
    /// `w:lvlJc @w:val` — label alignment.
    pub lvl_jc: Option<String>,
    /// Paragraph properties for the list item (indentation).
    pub ppr: Option<DocxPPr>,
    /// Run properties for the list label.
    pub rpr: Option<DocxRPr>,
}
