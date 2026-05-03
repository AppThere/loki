// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Stub texture type used in place of `wgpu::Texture` when the
//! `mock-texture` feature is active (the default for tests).

/// A lightweight stand-in for a GPU texture.
///
/// Stores only the dimensions needed for byte-budget calculations.
/// Replace with `wgpu::Texture` in a future session by swapping the
/// [`crate::PageTexture`] type alias and disabling this feature.
#[derive(Debug)]
pub struct MockTexture {
    /// Width of the texture in pixels.
    pub width: u32,
    /// Height of the texture in pixels.
    pub height: u32,
}

impl MockTexture {
    /// Returns the approximate GPU memory footprint in bytes (RGBA8 = 4 bytes
    /// per pixel, no mip-maps).
    #[must_use]
    pub fn byte_size(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4
    }
}
