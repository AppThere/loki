// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Stable shape identifiers.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A stable identifier for a shape within a drawing.
///
/// Used for selection, addressing in a CRDT bridge, and cross-referencing
/// (e.g. a presentation placeholder pointing at the shape that fills it).
/// Format is application-defined; importers typically reuse the source file's
/// ids.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ShapeId(String);

impl ShapeId {
    /// Wraps a string as a shape id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ShapeId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ShapeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for ShapeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
