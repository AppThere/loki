// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Editing-layer types and logic for the Loki document editor.
//!
//! This module contains cursor state representation, coordinate-space hit
//! testing, and the helper that derives a stable Loro [`Cursor`] from a
//! layout-based [`DocumentPosition`].
//!
//! [`Cursor`]: loro::Cursor

pub mod cursor;
pub mod hit_test;
