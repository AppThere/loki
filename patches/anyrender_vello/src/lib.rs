//! A [`vello`] backend for the [`anyrender`] 2D drawing abstraction
mod image_renderer;
mod scene;
mod window_renderer;

pub mod custom_paint_source;

pub use custom_paint_source::*;
pub use image_renderer::VelloImageRenderer;
pub use scene::VelloScenePainter;
pub use window_renderer::{VelloRendererOptions, VelloWindowRenderer};

pub use wgpu;

use std::num::NonZeroUsize;

// COMPAT(android-mali): Mali drivers (e.g. r54p2 on Pixel 9 / Mali-G715)
// crash with "Parent device is lost" when Vello compiles its shader modules
// from multiple threads concurrently. Force single-threaded shader init on
// Android, matching what upstream already does for macOS. Vello's own
// with_winit example applies the same Android workaround.
#[cfg(any(target_os = "macos", target_os = "android"))]
const DEFAULT_THREADS: Option<NonZeroUsize> = NonZeroUsize::new(1);
#[cfg(not(any(target_os = "macos", target_os = "android")))]
const DEFAULT_THREADS: Option<NonZeroUsize> = None;
