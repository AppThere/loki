// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared design tokens and Dioxus UI primitives for the Loki document suite.
//!
//! # Structure
//!
//! * [`tokens`] — design-token constants (colors, spacing, typography, layout).
//!   Import via `use loki_theme::tokens::*` or name individual sub-modules.
//! * [`components`] — shared Dioxus component primitives reusable across all
//!   Loki suite applications (toolbar buttons, icon buttons, etc.).

#![warn(missing_docs)]

pub mod components;
pub mod tokens;
