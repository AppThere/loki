// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Comment and annotation types.
//!
//! ODF and OOXML both support comments (annotations) that are anchored
//! to a range of content. The abstract model represents the anchor points
//! as [`CommentRef`] inline nodes and the comment content as [`Comment`].
//!
//! ODF: `office:annotation` / `office:annotation-end`.
//! OOXML: `w:commentRangeStart` / `w:commentRangeEnd` / `w:commentReference`.

/// The role of a [`CommentRef`] marker in the content flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CommentRefKind {
    /// Marks the start of the commented range.
    Start,
    /// Marks the end of the commented range.
    End,
    /// A single-point annotation with no range.
    Point,
}

/// An inline reference to a comment anchor point.
///
/// Appears in the content flow at the start and end of the commented range.
/// The actual comment content is stored in [`Comment`] and referenced by id.
///
/// ODF: `office:annotation` (start/point) / `office:annotation-end` (end).
/// OOXML: `w:commentRangeStart`, `w:commentRangeEnd`, `w:commentReference`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CommentRef {
    /// The comment identifier, matching a [`Comment::id`] in the document.
    pub id: String,
    /// Whether this marker is the start, end, or a point anchor.
    pub kind: CommentRefKind,
}

impl CommentRef {
    /// Creates a new [`CommentRef`] with the given id and kind.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: CommentRefKind) -> Self {
        Self {
            id: id.into(),
            kind,
        }
    }
}

/// The content and metadata of a document comment.
///
/// Comments are stored separately from the content flow; the content flow
/// contains only [`CommentRef`] anchors that reference a comment by id.
///
/// ODF: `office:annotation`. OOXML: `w:comment` in `word/comments.xml`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Comment {
    /// Unique comment identifier.
    pub id: String,

    /// The author of the comment.
    /// ODF: `dc:creator`. OOXML: `w:author`.
    pub author: Option<String>,

    /// The date the comment was created.
    /// ODF: `dc:date`. OOXML: `w:date`.
    pub date: Option<chrono::DateTime<chrono::Utc>>,

    /// The comment body as a sequence of block nodes.
    ///
    /// Stored as raw bytes to avoid a circular dependency between the
    /// annotation module and the block/inline modules. Consumers should
    /// parse these blocks using the crate's Block type.
    ///
    /// In practice, comment bodies are plain paragraphs. The raw storage
    /// avoids importing Block here, which would create a module cycle.
    pub body_raw: Vec<u8>,
}

impl Comment {
    /// Creates a new [`Comment`] with the given id and no content.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            author: None,
            date: None,
            body_raw: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_ref_start_end() {
        let start = CommentRef::new("c1", CommentRefKind::Start);
        let end = CommentRef::new("c1", CommentRefKind::End);
        assert_eq!(start.id, end.id);
        assert_ne!(start.kind, end.kind);
    }

    #[test]
    fn comment_author() {
        let mut c = Comment::new("c1");
        c.author = Some("Alice".into());
        assert_eq!(c.author.as_deref(), Some("Alice"));
    }
}
