// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! ODF specification version enumeration.
//!
//! [`OdfVersion`] represents the three active ODF versions and is used to
//! gate version-specific behaviour during import and export. The round-trip
//! rule requires that a document exported from a loaded file carries the same
//! version as the source file.

/// The ODF specification version of a document.
///
/// Used to gate version-specific behaviour during import and export,
/// and to enforce the round-trip rule: export at the same version
/// as import. ODF 1.3 §3 (`office:version` attribute).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OdfVersion {
    /// ODF 1.1 — ISO/IEC 26300:2006/Amd 1:2012.
    ///
    /// The `office:version` attribute may be absent; this is valid per the
    /// ODF 1.1 specification.
    V1_1,

    /// ODF 1.2 — ISO/IEC 26300-1:2015.
    ///
    /// The `office:version` attribute is mandatory per ODF 1.2 §2.2.1.
    V1_2,

    /// ODF 1.3 — OASIS Standard 2021.
    ///
    /// The `office:version` attribute is mandatory per ODF 1.3 §3.
    V1_3,
}

impl OdfVersion {
    /// Parse from the `office:version` attribute string.
    ///
    /// Returns `None` for an absent attribute (valid for ODF 1.1 documents).
    /// Returns `None` for any unrecognised version string; callers are
    /// responsible for deciding how to handle that case.
    ///
    /// ODF 1.3 §3 (`office:version` attribute).
    #[must_use]
    pub fn from_attr(s: &str) -> Option<Self> {
        match s {
            "1.1" => Some(Self::V1_1),
            "1.2" => Some(Self::V1_2),
            "1.3" => Some(Self::V1_3),
            _ => None,
        }
    }

    /// Returns the canonical `office:version` attribute string for this
    /// version. ODF 1.3 §3.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1_1 => "1.1",
            Self::V1_2 => "1.2",
            Self::V1_3 => "1.3",
        }
    }

    /// Whether the `office:version` attribute is mandatory in this version.
    ///
    /// Returns `true` for ODF 1.2 and later. ODF 1.1 does not require the
    /// attribute. ODF 1.2 §2.2.1.
    #[must_use]
    pub fn version_attr_required(self) -> bool {
        self >= Self::V1_2
    }

    /// Whether the label-alignment list-positioning model is available.
    ///
    /// ODF 1.2 introduced `text:list-level-position-and-space-mode` with the
    /// value `"label-alignment"` (ODF 1.2 §19.880). The legacy model
    /// (`text:space-before` etc.) must be used for ODF 1.1 documents.
    #[must_use]
    pub fn supports_label_alignment(self) -> bool {
        self >= Self::V1_2
    }
}

impl Default for OdfVersion {
    /// Defaults to [`OdfVersion::V1_3`], used for programmatically constructed
    /// documents with no [`loki_doc_model::io::DocumentSource`].
    fn default() -> Self {
        Self::V1_3
    }
}

#[cfg(test)]
mod tests {
    use super::OdfVersion;

    #[test]
    fn from_attr_known_versions() {
        assert_eq!(OdfVersion::from_attr("1.1"), Some(OdfVersion::V1_1));
        assert_eq!(OdfVersion::from_attr("1.2"), Some(OdfVersion::V1_2));
        assert_eq!(OdfVersion::from_attr("1.3"), Some(OdfVersion::V1_3));
    }

    #[test]
    fn from_attr_unknown_returns_none() {
        assert_eq!(OdfVersion::from_attr("2.0"), None);
        assert_eq!(OdfVersion::from_attr(""), None);
        assert_eq!(OdfVersion::from_attr("1.4"), None);
    }

    #[test]
    fn as_str_round_trips() {
        for v in [OdfVersion::V1_1, OdfVersion::V1_2, OdfVersion::V1_3] {
            assert_eq!(OdfVersion::from_attr(v.as_str()), Some(v));
        }
    }

    #[test]
    fn version_attr_required() {
        assert!(!OdfVersion::V1_1.version_attr_required());
        assert!(OdfVersion::V1_2.version_attr_required());
        assert!(OdfVersion::V1_3.version_attr_required());
    }

    #[test]
    fn supports_label_alignment() {
        assert!(!OdfVersion::V1_1.supports_label_alignment());
        assert!(OdfVersion::V1_2.supports_label_alignment());
        assert!(OdfVersion::V1_3.supports_label_alignment());
    }

    #[test]
    fn ordering() {
        assert!(OdfVersion::V1_1 < OdfVersion::V1_2);
        assert!(OdfVersion::V1_2 < OdfVersion::V1_3);
        assert!(OdfVersion::V1_1 < OdfVersion::V1_3);
    }

    #[test]
    fn default_is_v1_3() {
        assert_eq!(OdfVersion::default(), OdfVersion::V1_3);
    }
}
