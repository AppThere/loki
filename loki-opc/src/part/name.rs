// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Validated OPC part names: parsing and the grammar rules from §6.2.2.

use crate::error::{OpcError, OpcResult};

/// A validated OPC part name per ISO/IEC 29500-2:2021 §6.2.2.
///
/// A part name is a Unicode string starting with `/`, where each segment
/// between `/` separators satisfies the ABNF rules in §6.2.2. Part names
/// are case-preserved but compared case-insensitively per §6.3.5.
#[derive(Debug, Clone, Eq)]
pub struct PartName(String);

impl PartName {
    /// Parse and validate a part name string.
    /// Returns `OpcError::InvalidPartName` if the string does not conform
    /// to §6.2.2.
    ///
    /// The ABNF requires:
    /// - Starts with `/`
    /// - Does not end with `/`
    /// - Segments are non-empty
    /// - Segments are not `.` or `..` and do not end with `.`
    /// - No percent encoded `%2f` (`/`) or `%5c` (`\`)
    pub fn new(s: impl Into<String>) -> OpcResult<Self> {
        let s = s.into();

        if s.is_empty() {
            return Err(OpcError::InvalidPartName {
                name: s,
                reason: "Part name must not be empty",
            });
        }
        if !s.starts_with('/') {
            return Err(OpcError::InvalidPartName {
                name: s,
                reason: "Part name must start with /",
            });
        }
        if s.ends_with('/') {
            return Err(OpcError::InvalidPartName {
                name: s,
                reason: "Part name must not end with /",
            });
        }

        let segments = s.split('/').skip(1);
        for segment in segments {
            if segment.is_empty() {
                return Err(OpcError::InvalidPartName {
                    name: s,
                    reason: "Part name segments must not be empty (cannot contain //)",
                });
            }
            if segment == "." || segment == ".." {
                return Err(OpcError::InvalidPartName {
                    name: s,
                    reason: "Part name segments must not be '.' or '..'",
                });
            }
            if segment.ends_with('.') {
                return Err(OpcError::InvalidPartName {
                    name: s,
                    reason: "Part name segments must not end with '.'",
                });
            }
            let lower = segment.to_lowercase();
            if lower.contains("%2f") || lower.contains("%5c") {
                return Err(OpcError::InvalidPartName {
                    name: s,
                    reason: "Part name segments must not contain percent-encoded / or \\",
                });
            }
        }

        Ok(Self::new_unchecked(s))
    }

    /// Wraps an already-validated string as a [`PartName`] without re-running
    /// grammar validation.
    ///
    /// # Safety note
    /// Only for internal use where the name is known to be valid (e.g. ZIP
    /// entries already checked during read).
    pub fn new_unchecked(s: String) -> Self {
        Self(s)
    }

    /// Returns the string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the file extension, if any.
    ///
    /// Only the final segment is considered: a dotted directory segment
    /// (`/v1.0/data`) is a legal part name, and splitting the whole path
    /// would report the directory's suffix as the part's extension.
    pub fn extension(&self) -> Option<&str> {
        let final_segment = match self.0.rsplit_once('/') {
            Some((_, f)) => f,
            None => self.0.as_str(),
        };
        final_segment.rsplit_once('.').map(|(_, ext)| ext)
    }

    /// Returns the relationships part name for this part (§6.5.2).
    /// For `/foo/bar.xml` returns `/foo/_rels/bar.xml.rels`.
    pub fn relationships_part_name(&self) -> PartName {
        let name_str = self.as_str();
        let (dir, filename) = match name_str.rsplit_once('/') {
            Some((d, f)) => (d, f),
            None => ("", name_str),
        };
        // Unchecked is safe because the parent part was already validated.
        Self::new_unchecked(format!("{}/_rels/{}.rels", dir, filename))
    }
}

// Case-insensitive comparisons and hashing per §6.3.5.

impl PartialEq for PartName {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl std::hash::Hash for PartName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let lower = self.0.to_ascii_lowercase();
        lower.hash(state);
    }
}

impl std::fmt::Display for PartName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for PartName {
    type Error = OpcError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for PartName {
    type Error = OpcError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_part_names() {
        assert!(PartName::new("/word/document.xml").is_ok());
        assert!(PartName::new("/_rels/.rels").is_ok());
        assert!(PartName::new("/xl/worksheets/sheet1.xml").is_ok());
    }

    #[test]
    fn test_invalid_part_names() {
        assert!(matches!(
            PartName::new(""),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("word/document.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word//document.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word/document.xml/"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word/./document.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word/../document.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word./document.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word%2f/document.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
        assert!(matches!(
            PartName::new("/word/%5Cdocument.xml"),
            Err(OpcError::InvalidPartName { .. })
        ));
    }

    #[test]
    fn test_part_name_equivalence() {
        let a = PartName::new("/Foo/Bar.xml").unwrap();
        let b = PartName::new("/foo/bar.xml").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_extension() {
        let a = PartName::new("/word/document.xml").unwrap();
        assert_eq!(a.extension(), Some("xml"));
    }

    #[test]
    fn extension_reads_the_final_segment_not_the_whole_path() {
        // A dotted directory segment is a legal part name (`v1.0` neither is
        // nor ends with a dot), so splitting the whole path on its last '.'
        // reports the directory's suffix as the extension. `extension()` feeds
        // `ContentTypeMap::resolve` for every part, so a wrong answer here
        // mis-types the part.
        let dotted_dir = PartName::new("/v1.0/data").unwrap();
        assert_eq!(dotted_dir.extension(), None);

        // The same trap with a real extension present: the dot that counts is
        // the one in the final segment.
        let both = PartName::new("/a.b/c.xml").unwrap();
        assert_eq!(both.extension(), Some("xml"));

        // A leading-dot final segment keeps its extension (`/_rels/.rels`).
        let rels = PartName::new("/_rels/.rels").unwrap();
        assert_eq!(rels.extension(), Some("rels"));
    }

    #[test]
    fn test_relationships_part_name() {
        let doc = PartName::new("/word/document.xml").unwrap();
        assert_eq!(
            doc.relationships_part_name().as_str(),
            "/word/_rels/document.xml.rels"
        );

        let root = PartName::new("/image.png").unwrap();
        assert_eq!(
            root.relationships_part_name().as_str(),
            "/_rels/image.png.rels"
        );
    }
}
