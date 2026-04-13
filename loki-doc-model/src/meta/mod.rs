// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document metadata: core properties, custom properties, and language tags.
//!
//! Both ODF (`<office:meta>`) and OOXML (`docProps/core.xml`) define a set
//! of core document properties. TR 29166 §7.2.1 describes the structural
//! correspondence between the two formats.

pub mod core;
pub mod language;

pub use core::{CustomProperty, CustomPropertyValue, DocumentMeta};
pub use language::LanguageTag;
