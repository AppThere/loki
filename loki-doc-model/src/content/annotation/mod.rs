// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Comment and change-tracking annotation types.
//!
//! Both ODF and OOXML support comments anchored to content ranges and
//! tracked changes (insertions, deletions, format changes). This module
//! provides the abstract representations.
//!
//! TR 29166 §7.2.7 and ADR-0006.

pub mod comment;
pub mod tracked;

pub use comment::{Comment, CommentRef, CommentRefKind};
pub use tracked::{TrackedChange, TrackedChangeKind};
