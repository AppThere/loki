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

//! Intermediate Rust structs mirroring the raw OOXML XML structure.
//!
//! These types are the deserialization targets for `quick-xml` event parsing.
//! They closely mirror the XML element hierarchy defined in ECMA-376. They are
//! **crate-internal** — never exposed in the public API. The mapper layer
//! translates these into [`loki_doc_model`] types.

pub mod document;
pub mod fields;
pub mod footnotes;
pub mod numbering;
pub mod paragraph;
pub mod settings;
pub mod styles;
