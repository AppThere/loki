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

//! Document field types — inline dynamic content.
//!
//! Fields represent content that is evaluated dynamically at render time:
//! page numbers, dates, cross-references, etc.
//! TR 29166 §5.2.19 and ADR-0005.

pub mod types;

pub use types::{CrossRefFormat, Field, FieldKind};
