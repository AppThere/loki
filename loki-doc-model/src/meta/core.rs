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

//! Document metadata: core properties and custom properties.
//!
//! Both ODF (`<office:meta>`) and OOXML (`docProps/core.xml` and
//! `docProps/app.xml`) define a set of core document properties.
//! TR 29166 §7.2.1 describes the structural correspondence.

use super::language::LanguageTag;

/// Core document metadata.
///
/// Maps to ODF `<office:meta>` and OOXML `docProps/core.xml` /
/// `docProps/app.xml`. TR 29166 §7.2.1.
///
/// All fields are `Option<T>`; missing metadata is represented as `None`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentMeta {
    /// The document title.
    /// ODF: `dc:title`. OOXML: `cp:coreProperties/dc:title`.
    pub title: Option<String>,

    /// The document subject.
    /// ODF: `dc:subject`. OOXML: `cp:coreProperties/dc:subject`.
    pub subject: Option<String>,

    /// A comma-separated list of keywords describing the document.
    /// ODF: `meta:keyword`. OOXML: `cp:coreProperties/cp:keywords`.
    pub keywords: Option<String>,

    /// A short description of the document.
    /// ODF: `dc:description`. OOXML: `cp:coreProperties/dc:description`.
    pub description: Option<String>,

    /// The document's initial creator / author.
    /// ODF: `meta:initial-creator`. OOXML: `cp:coreProperties/dc:creator`.
    pub creator: Option<String>,

    /// The person who last modified the document.
    /// ODF: `dc:creator`. OOXML: `cp:coreProperties/cp:lastModifiedBy`.
    pub last_modified_by: Option<String>,

    /// When the document was created (UTC).
    /// ODF: `meta:creation-date`. OOXML: `cp:coreProperties/dcterms:created`.
    pub created: Option<chrono::DateTime<chrono::Utc>>,

    /// When the document was last modified (UTC).
    /// ODF: `dc:date`. OOXML: `cp:coreProperties/dcterms:modified`.
    pub modified: Option<chrono::DateTime<chrono::Utc>>,

    /// The default language of the document content.
    /// ODF: `dc:language`. OOXML: `w:settings/w:defaultTab` language.
    /// TR 29166 §6.2.6.
    pub language: Option<LanguageTag>,

    /// Revision number / save count.
    /// ODF: `meta:editing-cycles`. OOXML: `cp:coreProperties/cp:revision`.
    pub revision: Option<u32>,

    /// Total editing time in minutes.
    /// ODF: `meta:editing-duration`. OOXML: `cp:coreProperties/cp:totalEditingTime`.
    pub editing_duration_minutes: Option<u64>,

    /// Custom (user-defined) document properties.
    pub custom_properties: Vec<CustomProperty>,
}

/// A user-defined document property.
///
/// ODF: `<meta:user-defined meta:name="...">value</meta:user-defined>`.
/// OOXML: `<property name="...">` in `docProps/custom.xml`.
/// TR 29166 §7.2.1.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CustomProperty {
    /// The property name.
    pub name: String,
    /// The property value, represented as a typed variant.
    pub value: CustomPropertyValue,
}

/// The typed value of a custom document property.
///
/// ODF and OOXML both support a limited set of value types for custom
/// properties. TR 29166 §7.2.1.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CustomPropertyValue {
    /// A plain text string.
    Text(String),
    /// A 64-bit floating-point number.
    Number(f64),
    /// A boolean value.
    Bool(bool),
    /// A date-time value (UTC).
    DateTime(chrono::DateTime<chrono::Utc>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_meta_default_is_empty() {
        let meta = DocumentMeta::default();
        assert!(meta.title.is_none());
        assert!(meta.creator.is_none());
        assert!(meta.custom_properties.is_empty());
    }

    #[test]
    fn custom_property_text() {
        let prop = CustomProperty {
            name: "Department".into(),
            value: CustomPropertyValue::Text("Engineering".into()),
        };
        assert_eq!(prop.name, "Department");
    }
}
