// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Vello renderer construction for the standalone `PageSource` impl.
//!
//! **One caller, deliberately.** `LokiPageSource` — the Blitz canvas path, and
//! the one that actually runs in the apps — used to build a renderer through
//! here too. It now borrows Blitz's own renderer during the paint callback
//! (`CustomPaintCtx::renderer_mut`), because each `vello::Renderer` carries
//! ~165 MiB of fixed scratch buffers regardless of scene complexity and a second
//! instance bought nothing for that. What remains is
//! `page_source_impl.rs`'s standalone `impl PageSource for DocPageSource`, which
//! has no Blitz context to borrow from.
//!
//! The helper stays because that caller still needs the Android COMPAT flags in
//! one place.

use std::num::NonZeroUsize;

use vello::{AaSupport, RendererOptions};

/// Create a `vello::Renderer` with Loki's platform configuration.
pub(crate) fn create_vello_renderer(
    device: &wgpu::Device,
) -> Result<vello::Renderer, vello::Error> {
    vello::Renderer::new(
        device,
        RendererOptions {
            // COMPAT(android-mali): Mali r54 drivers (Pixel 9 / Mali-G715)
            // lose the Vulkan device executing Vello's compute dispatches.
            // use_cpu runs the compute stages on the CPU; fine rasterization
            // stays on the GPU.
            #[cfg(target_os = "android")]
            use_cpu: true,
            #[cfg(not(target_os = "android"))]
            use_cpu: false,
            // COMPAT(android-mali): Mali drivers (Pixel 9 / Mali-G715) lose
            // the Vulkan device executing Vello's MSAA fine-raster pipelines;
            // compile only the area-AA variants on Android.
            #[cfg(target_os = "android")]
            antialiasing_support: AaSupport::area_only(),
            #[cfg(not(target_os = "android"))]
            antialiasing_support: AaSupport::all(),
            num_init_threads: NonZeroUsize::new(1),
            pipeline_cache: None,
        },
    )
}
