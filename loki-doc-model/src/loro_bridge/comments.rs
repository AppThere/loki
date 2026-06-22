// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document comments in the Loro CRDT.
//!
//! Comments (annotation bodies, keyed by id) are stored as a lossless `serde`
//! JSON snapshot under [`KEY_COMMENTS_JSON`] in the comments map — the same
//! strategy used for metadata and the style catalog. The in-flow
//! [`crate::content::annotation::CommentRef`] anchors live in the block text
//! and round-trip with the blocks; only the bodies need separate storage.

use loro::{LoroDoc, LoroMap};

use super::BridgeError;
use crate::content::annotation::Comment;
use crate::loro_schema::{KEY_COMMENTS, KEY_COMMENTS_JSON};

/// Writes `comments` into the comments `map` as a JSON snapshot.
pub(super) fn write_comments(comments: &[Comment], map: &LoroMap) -> Result<(), BridgeError> {
    write_comments_json(comments, map);
    Ok(())
}

#[cfg(feature = "serde")]
fn write_comments_json(comments: &[Comment], map: &LoroMap) {
    match serde_json::to_string(comments) {
        Ok(json) => {
            if let Err(err) = map.insert(KEY_COMMENTS_JSON, json) {
                tracing::warn!("loro bridge: failed to store comments snapshot: {err}");
            }
        }
        // Unreachable in practice: Comment derives Serialize.
        Err(err) => tracing::warn!("loro bridge: failed to serialize comments: {err}"),
    }
}

#[cfg(not(feature = "serde"))]
fn write_comments_json(_comments: &[Comment], _map: &LoroMap) {
    tracing::warn!(
        "loro bridge: comments not persisted — build loki-doc-model with the \
         `serde` feature (default) to round-trip document comments"
    );
}

/// Reads the comments from the comments `map`. Empty when absent.
pub(super) fn read_comments(map: &LoroMap) -> Vec<Comment> {
    read_comments_json(map).unwrap_or_default()
}

#[cfg(feature = "serde")]
fn read_comments_json(map: &LoroMap) -> Option<Vec<Comment>> {
    let json = map
        .get(KEY_COMMENTS_JSON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())?;
    serde_json::from_str(&json).ok()
}

#[cfg(not(feature = "serde"))]
fn read_comments_json(_map: &LoroMap) -> Option<Vec<Comment>> {
    None
}

/// Overwrites the document-level comments stored in `loro` with `comments`.
///
/// Used by the editor to persist comment edits as a CRDT mutation, so they
/// survive incremental rebuilds, undo/redo, and Loro import/export. The caller
/// is responsible for committing.
pub fn write_document_comments(loro: &LoroDoc, comments: &[Comment]) -> Result<(), BridgeError> {
    let map = loro.get_map(KEY_COMMENTS);
    write_comments(comments, &map)
}

/// Reads the document-level comments stored in `loro`.
#[must_use]
pub fn read_document_comments(loro: &LoroDoc) -> Vec<Comment> {
    let map = loro.get_map(KEY_COMMENTS);
    read_comments(&map)
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::content::block::Block;
    use crate::content::inline::Inline;

    #[test]
    fn comments_round_trip_through_loro() {
        let doc = LoroDoc::new();
        let mut comment = Comment::new("c1").with_plain_body("First line\nSecond line");
        comment.author = Some("Reviewer".into());
        let comments = vec![comment];

        write_document_comments(&doc, &comments).unwrap();
        let back = read_document_comments(&doc);

        assert_eq!(back.len(), 1);
        assert_eq!(back[0].id, "c1");
        assert_eq!(back[0].author.as_deref(), Some("Reviewer"));
        assert_eq!(back[0].body.len(), 2, "two body paragraphs");
        assert!(
            matches!(&back[0].body[0], Block::Para(i) if matches!(&i[0], Inline::Str(s) if s == "First line"))
        );
    }

    #[test]
    fn empty_when_absent() {
        let doc = LoroDoc::new();
        assert!(read_document_comments(&doc).is_empty());
    }
}
