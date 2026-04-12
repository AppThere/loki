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

//! ODF text-field model types.
//!
//! Covers the field elements under `text:*` described in ODF 1.3 §12
//! (Text Fields). Fields are inline content within a paragraph that
//! produce computed values (page numbers, dates, document properties, etc.).

/// An ODF text field, representing a computed inline value.
///
/// ODF 1.3 §12 (Text Fields). Each variant corresponds to one or more
/// `text:*` field elements from the ODF specification.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OdfField {
    /// Current page number. ODF 1.3 §12.2 `text:page-number`.
    ///
    /// `select_page` corresponds to `text:select-page`: `"current"`,
    /// `"previous"`, or `"next"`.
    PageNumber {
        /// Which page to count: `"current"`, `"previous"`, or `"next"`.
        select_page: Option<String>,
    },

    /// Total number of pages in the document.
    /// ODF 1.3 §12.4 `text:page-count`.
    PageCount,

    /// Current date. ODF 1.3 §12.6 `text:date`.
    Date {
        /// Data-style name controlling formatting.
        data_style: Option<String>,
        /// Fixed ISO-8601 date value (when `text:fixed="true"`).
        fixed_value: Option<String>,
    },

    /// Current time. ODF 1.3 §12.7 `text:time`.
    Time {
        /// Data-style name controlling formatting.
        data_style: Option<String>,
        /// Fixed ISO-8601 time value (when `text:fixed="true"`).
        fixed_value: Option<String>,
    },

    /// Document title from metadata. ODF 1.3 §12.13 `text:title`.
    Title,

    /// Document subject from metadata. ODF 1.3 §12.14 `text:subject`.
    Subject,

    /// Author (first-name + last-name) from metadata.
    /// ODF 1.3 §12.16 `text:author-name`.
    AuthorName,

    /// Document file name. ODF 1.3 §12.21 `text:file-name`.
    ///
    /// `display` reflects `text:display`: `"full"`, `"path"`, `"name"`,
    /// `"name-and-extension"`.
    FileName {
        /// Which part of the path to display.
        display: Option<String>,
    },

    /// Word count. ODF 1.3 §12.27 `text:word-count`.
    WordCount,

    /// Heading / chapter name. ODF 1.3 §12.19 `text:chapter`.
    ///
    /// `display_levels` reflects `text:display-levels` (1–10).
    ChapterName {
        /// How many outline levels to include.
        display_levels: u8,
    },

    /// Cross-reference to a named target. ODF 1.3 §12.32 `text:bookmark-ref`
    /// and related elements.
    CrossReference {
        /// Target bookmark or sequence name.
        ref_name: String,
        /// What to display (e.g. `"text"`, `"number"`, `"page"`).
        display: Option<String>,
    },

    /// A field element not specifically modelled above.
    ///
    /// The local XML element name is preserved for diagnostic purposes.
    Unknown {
        /// Local element name (without namespace prefix).
        local_name: String,
        /// The text content of the field element, if any.
        current_value: Option<String>,
    },
}
