// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Relationships mapping and resolution configuration handling targeting components within elements recursively.

mod location;
mod parse;
mod write;

use crate::error::OpcResult;

pub use location::{package_relationships_part, relationships_part_for};
pub use parse::parse_relationships_part;
pub use write::write_relationships_part;

/// Whether the target is internal to the package or external (§6.5.3).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetMode {
    /// Mapping logically to paths inside the contained zip file.
    #[default]
    Internal,
    /// Absolute or independent URIs linking external resources blindly.
    External,
}

/// A single OPC relationship per §6.5.3.
#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    /// Unique within the relationships part. Format is application-defined;
    /// conventionally `rId1`, `rId2`, etc.
    pub id: String,
    /// An IRI identifying the relationship type (§6.5.3).
    pub rel_type: String,
    /// The target IRI. Relative for Internal relationships.
    pub target: String,
    /// Whether the target is internal to the package or external (§6.5.3).
    pub target_mode: TargetMode,
}

/// A set of OPC relationships from a single source (package or part).
/// Corresponds to a single `.rels` XML file per §6.5.
#[derive(Debug, Clone, Default)]
pub struct RelationshipSet {
    relationships: Vec<Relationship>,
}

impl RelationshipSet {
    /// Retrieves a specific relationship uniquely by its string layout.
    pub fn get(&self, id: &str) -> Option<&Relationship> {
        self.relationships.iter().find(|r| r.id == id)
    }

    /// Provides immutable list extraction iterative streams.
    pub fn iter(&self) -> impl Iterator<Item = &Relationship> {
        self.relationships.iter()
    }

    /// Extrapolates relationships filtered rigidly by specific format type values.
    pub fn by_type<'a>(&'a self, rel_type: &'a str) -> impl Iterator<Item = &'a Relationship> {
        self.relationships.iter().filter(move |r| r.rel_type == rel_type)
    }

    /// Mounts additional relationships inside the struct mapping.
    pub fn add(&mut self, rel: Relationship) -> OpcResult<()> {
        self.relationships.push(rel);
        Ok(())
    }

    /// Expunges relationship identifiers removing their associations directly.
    pub fn remove(&mut self, id: &str) -> Option<Relationship> {
        if let Some(pos) = self.relationships.iter().position(|x| x.id == id) {
            Some(self.relationships.remove(pos))
        } else {
            None
        }
    }

    /// Indication if local mappings contain elements correctly.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.relationships.is_empty()
    }

    /// Returns size metrics for active metadata maps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.relationships.len()
    }

    /// Generate a unique `Id` value suitable for a new relationship.
    /// Format: `rId1`, `rId2`, ... incrementing from the current maximum.
    #[must_use]
    pub fn next_id(&self) -> String {
        let max_id = self.relationships.iter().filter_map(|r| {
            if r.id.starts_with("rId") {
                r.id[3..].parse::<u32>().ok()
            } else {
                None
            }
        }).max().unwrap_or(0);
        format!("rId{}", max_id + 1)
    }
}
