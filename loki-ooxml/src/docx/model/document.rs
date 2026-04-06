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

//! Intermediate model for `word/document.xml`.
//!
//! Mirrors ECMA-376 §17.2 (document structure) and §17.3 (block-level content).

use super::paragraph::{DocxParagraph, DocxSectPr};
use super::styles::DocxTableModel;

/// Top-level intermediate model for `w:document` (ECMA-376 §17.2.3).
#[derive(Debug, Clone, Default)]
pub struct DocxDocument {
    /// The document body.
    pub body: DocxBody,
}

/// Intermediate model for `w:body` (ECMA-376 §17.2.2).
#[derive(Debug, Clone, Default)]
pub struct DocxBody {
    /// Ordered block-level content children.
    pub children: Vec<DocxBodyChild>,
    /// Final section properties (`w:sectPr` as last child of `w:body`).
    /// ECMA-376 §17.6.17.
    pub final_sect_pr: Option<DocxSectPr>,
}

/// A block-level child of `w:body`.
#[derive(Debug, Clone)]
pub enum DocxBodyChild {
    /// A paragraph (`w:p`). ECMA-376 §17.3.1.22.
    Paragraph(DocxParagraph),
    /// A table (`w:tbl`). ECMA-376 §17.4.
    Table(DocxTableModel),
    /// A structured document tag (`w:sdt`) — stored opaquely for now.
    Sdt,
}
