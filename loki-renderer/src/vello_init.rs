// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared Vello renderer construction.
//!
//! Both render paths (`LokiPageSource` for Blitz canvases and the standalone
//! `PageSource` impl) need a `vello::Renderer` configured with the same
//! Android workarounds; this helper keeps the COMPAT flags in one place.

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
