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

//! Document metadata: core properties, custom properties, and language tags.
//!
//! Both ODF (`<office:meta>`) and OOXML (`docProps/core.xml`) define a set
//! of core document properties. TR 29166 §7.2.1 describes the structural
//! correspondence between the two formats.

pub mod core;
pub mod language;

pub use core::{CustomProperty, CustomPropertyValue, DocumentMeta};
pub use language::LanguageTag;
