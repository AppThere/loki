// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Design-token constants for the Loki visual language.
//!
//! All visual constants live in the sub-modules below and are re-exported
//! from this module for ergonomic wildcard imports:
//!
//! ```rust
//! use loki_theme::tokens::*;
//! ```
//!
//! Component files **must not** embed magic numbers; reference these constants
//! instead to maintain a single source of truth.

pub mod colors;
pub mod layout;
pub mod spacing;
pub mod typography;

pub use colors::*;
pub use layout::*;
pub use spacing::*;
pub use typography::*;
