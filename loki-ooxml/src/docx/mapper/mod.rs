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
//! This layer is the second step of the two-step DOCX import pipeline.
//! Mapper functions accept the crate-internal [`super::model`] structs
//! and produce format-neutral [`loki_doc_model`] values.
//!
//! Implementation is pending for v0.2.0.
