// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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
