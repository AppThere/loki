// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tiered page render cache policy for Loki's Vello renderer.
//!
//! This crate provides the pure-logic layer that decides which cache tier each
//! document page belongs to based on scroll position. No GPU, no windowing —
//! just data structures and algorithms.
//!
//! # Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  Hot zone   │ 2 × viewport height centred on the visible area   │
//! │  Warm zone  │ Hot zone ± 3 × viewport height                    │
//! │  Cold zone  │ everything else                                    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! When scrolling stops for [`scroll_state::SETTLE_DURATION`], the phase
//! transitions to [`scroll_state::ScrollPhase::Settling`] and the cache
//! should begin upgrading pages in the hot zone to full resolution.

pub mod scroll_state;
pub mod tier_policy;

pub use scroll_state::{SETTLE_DURATION, ScrollPhase, ScrollState};
pub use tier_policy::{CacheTier, PageGeometry, assign_tier};
