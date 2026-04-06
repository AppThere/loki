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

//! Document content model: blocks, inlines, tables, fields, and annotations.
//!
//! The content layer is inspired by `Text.Pandoc.Definition`'s `Block` and
//! `Inline` hierarchy, which has been validated against 30+ formats over
//! 18 years. Office-document-specific content types are added as extensions.
//! See ADR-0001.

pub mod annotation;
pub mod attr;
pub mod block;
pub mod field;
pub mod inline;
pub mod table;

pub use attr::{ExtensionBag, ExtensionKey, NodeAttr};
pub use block::Block;
pub use inline::Inline;
