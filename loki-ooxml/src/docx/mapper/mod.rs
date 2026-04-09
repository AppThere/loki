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

//! Translates intermediate OOXML model types → [`loki_doc_model`] types.
//!
//! This is the second step of the two-step DOCX import pipeline:
//!
//! 1. **XML → intermediate model** (`reader` layer)
//! 2. **Intermediate model → [`loki_doc_model`]** (this layer)
//!
//! Entry point: [`document::map_document`].

pub(crate) mod document;
pub(crate) mod images;
pub(crate) mod inline;
pub(crate) mod numbering;
pub(crate) mod paragraph;
pub(crate) mod props;
pub(crate) mod styles;
pub(crate) mod table;
