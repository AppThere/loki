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

//! XML readers for ODF document parts.
//!
//! Each reader accepts raw XML bytes (`&[u8]`) and returns the corresponding
//! intermediate model type.  All readers set `trim_text(false)` so that
//! significant whitespace inside `text:span` and similar elements is
//! preserved verbatim.

pub(crate) mod meta;
pub(crate) mod styles;
