// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document metadata: core properties, custom properties, and language tags.
//!
//! Both ODF (`<office:meta>`) and OOXML (`docProps/core.xml`) define a set
//! of core document properties. TR 29166 §7.2.1 describes the structural
//! correspondence between the two formats.

pub mod core;
pub mod dublin_core;
pub mod language;

pub use core::{CustomProperty, CustomPropertyValue, DocumentMeta};
pub use dublin_core::{DCMI_TYPE_TEXT, DublinCoreMeta};
pub use language::LanguageTag;
