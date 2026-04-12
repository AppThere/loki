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

//! Intermediate ODF document model.
//!
//! These types are **internal only** — they mirror the XML structure of an ODF
//! package and are used as an intermediate representation between the raw XML
//! parser and the format-neutral [`loki_doc_model`] types.
//!
//! The model is deliberately close to the ODF 1.3 element tree so that
//! spec-section references remain accurate and round-trip fidelity is
//! preserved. Conversion to `loki_doc_model` types happens in a separate
//! mapping step.
//!
//! All types are defined now but consumed only by the XML parser and mapper
//! added in later sessions; the dead-code lint is suppressed at the module
//! level to keep the build warning-free.
#![allow(dead_code)]

pub(crate) mod document;
pub(crate) mod fields;
pub(crate) mod frames;
pub(crate) mod list_styles;
pub(crate) mod notes;
pub(crate) mod paragraph;
pub(crate) mod styles;
pub(crate) mod tables;
