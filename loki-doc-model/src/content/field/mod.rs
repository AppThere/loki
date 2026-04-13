// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document field types — inline dynamic content.
//!
//! Fields represent content that is evaluated dynamically at render time:
//! page numbers, dates, cross-references, etc.
//! TR 29166 §5.2.19 and ADR-0005.

pub mod types;

pub use types::{CrossRefFormat, Field, FieldKind};
