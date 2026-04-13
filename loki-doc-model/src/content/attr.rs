// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Generic node attributes and format-specific extension storage.
//!
//! [`NodeAttr`] is modelled directly on the `Attr` type in
//! `Text.Pandoc.Definition`: `Attr = (Text, [Text], [(Text, Text)])`.
//! [`ExtensionBag`] provides opaque storage for format-specific data
//! that has no abstract model representation, as described in
//! TR 29166 §8.3.3.

/// Opaque key for an entry in an [`ExtensionBag`].
///
/// The `namespace` field identifies the source format
/// (e.g. `"odf:1.3"`, `"ooxml:strict"`); the `tag` field identifies
/// the element or attribute within that format.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtensionKey {
    /// Format namespace identifier. E.g. `"odf:1.3"`, `"ooxml:strict"`.
    pub namespace: String,
    /// Element or attribute tag within that namespace.
    pub tag: String,
}

impl ExtensionKey {
    /// Creates a new [`ExtensionKey`].
    #[must_use]
    pub fn new(namespace: impl Into<String>, tag: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            tag: tag.into(),
        }
    }
}

/// Opaque storage for format-specific data that has no abstract model
/// representation.
///
/// TR 29166 §8.3.3 classifies change tracking, custom XML, and certain
/// field constructs as "difficult" translations. These are preserved here
/// without interpretation to enable lossless round-trips within a format.
///
/// Entries are keyed by [`ExtensionKey`] (namespace + tag) and stored as
/// raw bytes. The interpretation of the bytes is the responsibility of the
/// format-specific importer/exporter.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtensionBag {
    entries: Vec<(ExtensionKey, Vec<u8>)>,
}

impl ExtensionBag {
    /// Creates an empty [`ExtensionBag`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces an entry. If an entry with the same key already
    /// exists, its value is replaced and the old value is returned.
    pub fn insert(&mut self, key: ExtensionKey, value: Vec<u8>) -> Option<Vec<u8>> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            let old = self.entries[pos].1.clone();
            self.entries[pos].1 = value;
            Some(old)
        } else {
            self.entries.push((key, value));
            None
        }
    }

    /// Returns a reference to the value for the given key, if present.
    #[must_use]
    pub fn get(&self, key: &ExtensionKey) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_slice())
    }

    /// Removes the entry for the given key and returns its value, if present.
    pub fn remove(&mut self, key: &ExtensionKey) -> Option<Vec<u8>> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    /// Returns `true` if the bag contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of entries in the bag.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Iterates over all entries as `(&ExtensionKey, &[u8])` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&ExtensionKey, &[u8])> {
        self.entries.iter().map(|(k, v)| (k, v.as_slice()))
    }
}

/// Generic attribute bag attached to content nodes.
///
/// Modelled on the `Attr` type in `Text.Pandoc.Definition`.
/// Provides an identifier, a list of semantic classes, and arbitrary
/// key-value pairs for format-neutral metadata. Format-specific opaque
/// data lives in `extensions`.
///
/// ODF: `xml:id` maps to `id`; class-like attributes (e.g. custom styles
/// expressed as class names) map to `classes`.
/// OOXML: `w:rsidR` (revision save ID) or `w:bookmarkStart` `w:id`
/// attributes map to `id`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeAttr {
    /// Optional identifier. Maps to `xml:id` in ODF.
    pub id: Option<String>,
    /// Semantic class names (e.g. `"keep-with-next"`, `"page-break-before"`).
    pub classes: Vec<String>,
    /// Arbitrary key-value pairs for format-neutral metadata.
    pub kv: Vec<(String, String)>,
    /// Opaque format-specific extension data.
    pub extensions: ExtensionBag,
}

impl NodeAttr {
    /// Creates an empty [`NodeAttr`] with no id, classes, kv, or extensions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`NodeAttr`] with just an id.
    #[must_use]
    pub fn with_id(id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_bag_insert_get_remove() {
        let mut bag = ExtensionBag::new();
        let key = ExtensionKey::new("odf:1.3", "custom:field");
        assert!(bag.get(&key).is_none());

        let old = bag.insert(key.clone(), b"hello".to_vec());
        assert!(old.is_none());
        assert_eq!(bag.get(&key), Some(b"hello".as_slice()));

        let old2 = bag.insert(key.clone(), b"world".to_vec());
        assert_eq!(old2.as_deref(), Some(b"hello".as_slice()));
        assert_eq!(bag.get(&key), Some(b"world".as_slice()));

        let removed = bag.remove(&key);
        assert_eq!(removed.as_deref(), Some(b"world".as_slice()));
        assert!(bag.is_empty());
    }

    #[test]
    fn node_attr_default_is_empty() {
        let attr = NodeAttr::default();
        assert!(attr.id.is_none());
        assert!(attr.classes.is_empty());
        assert!(attr.kv.is_empty());
        assert!(attr.extensions.is_empty());
    }
}
