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

//! DOCX (WordprocessingML) import for the Loki document suite.
//!
//! ECMA-376 Part 1, §17 — WordprocessingML.
//!
//! # Architecture
//!
//! Parsing proceeds in two steps:
//!
//! 1. **XML → intermediate model** ([`model`] + [`reader`]): `quick-xml`
//!    event-mode parsers populate crate-internal structs that mirror the raw
//!    OOXML XML hierarchy.
//!
//! 2. **Intermediate model → [`loki_doc_model`]** ([`mapper`]): the mapper
//!    layer translates the intermediate structs into the format-neutral
//!    abstract document model.

pub mod export;
pub mod import;
pub(crate) mod mapper;
pub(crate) mod model;
pub(crate) mod reader;
