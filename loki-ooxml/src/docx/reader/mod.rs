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

//! XML readers that parse OOXML part bytes into intermediate model types.
//!
//! All readers use `quick-xml` in event reader mode with `trim_text(false)`
//! to preserve whitespace. See ADR-0002.

pub mod document;
pub mod footnotes;
pub mod numbering;
pub mod settings;
pub mod styles;
pub mod util;

