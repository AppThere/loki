// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared Dioxus component primitives for the Loki document suite.
//!
//! Components placed here must be application-agnostic — they must not
//! reference any application-specific route enum or business logic.
//!
//! # Planned components
//!
//! The following primitives are planned for future extraction once multiple
//! suite apps share the same pattern:
//!
//! * `ToolbarButton` — icon or text button styled to fit inside a toolbar.
//! * `IconButton` — square icon-only button with hover feedback.
//! * `StatusChip` — small badge for page counts, zoom levels, etc.
