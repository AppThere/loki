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

//! Page layout, sections, and headers/footers.
//!
//! TR 29166 §7.2.8 classifies section and page layout as "moderate to
//! difficult" translation between ODF and OOXML. This module covers the
//! common subset. See the individual submodule documentation for details.

pub mod header_footer;
pub mod page;
pub mod section;

pub use header_footer::{HeaderFooter, HeaderFooterKind};
pub use page::{PageLayout, PageMargins, PageOrientation, PageSize, SectionColumns};
pub use section::Section;
