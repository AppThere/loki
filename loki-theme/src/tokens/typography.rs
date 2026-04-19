// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Typography scale design tokens (CSS pixels as `f32`).

// Token constants may not all be referenced in every build state.
#![allow(dead_code)]

/// Body text — paragraph copy and list content.
pub const FONT_SIZE_BODY: f32 = 14.0;

/// Label / caption — metadata, timestamps, and secondary info.
pub const FONT_SIZE_LABEL: f32 = 12.0;

/// Heading — section titles within a screen.
pub const FONT_SIZE_HEADING: f32 = 20.0;
