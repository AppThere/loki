// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! BCP 47 language tag representation.
//!
//! Both ODF (`fo:language`, `fo:country`, `fo:script`) and OOXML
//! (`w:lang`) identify languages using BCP 47 tags. TR 29166 §6.2.6
//! describes the language metadata mapping between the two formats.

use std::fmt;

/// A BCP 47 language tag.
///
/// Used in [`crate::style::props::CharProps`] and
/// [`crate::meta::DocumentMeta`] to identify the natural language of
/// text content. TR 29166 §6.2.6 "Language" feature.
///
/// The tag is stored as a normalized string (e.g. `"en-US"`, `"fr-FR"`,
/// `"zh-Hans-CN"`). No structural validation beyond non-emptiness is
/// enforced at the model level; format importers are responsible for
/// producing well-formed tags.
///
/// # Examples
///
/// ```
/// use loki_doc_model::meta::LanguageTag;
///
/// let tag = LanguageTag::new("en-US");
/// assert_eq!(tag.as_str(), "en-US");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LanguageTag(String);

impl LanguageTag {
    /// Creates a new [`LanguageTag`] from the given BCP 47 string.
    ///
    /// The value is stored as provided; callers should supply a
    /// syntactically correct BCP 47 tag (e.g. `"en-US"`).
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }

    /// Returns the language tag as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for LanguageTag {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for LanguageTag {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for LanguageTag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_display() {
        let tag = LanguageTag::new("en-US");
        assert_eq!(tag.to_string(), "en-US");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn from_str_ref() {
        let tag: LanguageTag = "fr-FR".into();
        assert_eq!(tag.as_str(), "fr-FR");
    }
}
