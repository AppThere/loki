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

//! Named style definitions and the style catalog.
//!
//! Both ODF and OOXML maintain a catalog of named styles. This module
//! provides the abstract representation. TR 29166 §7.2.3.
//!
//! See ADR-0007 for the rationale behind using `IndexMap` in the catalog.

pub mod catalog;
pub mod char_style;
pub mod list_style;
pub mod para_style;
pub mod props;
pub mod table_style;

pub use catalog::{ResolvedCharProps, ResolvedParaProps, StyleCatalog, StyleId};
pub use char_style::CharacterStyle;
pub use list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
pub use para_style::ParagraphStyle;
pub use table_style::TableStyle;
