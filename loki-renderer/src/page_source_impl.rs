// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`PageSource`] implementation for [`DocPageSource`].
//!
//! Split from `doc_page_source.rs` to stay under the file-size ceiling.  This
//! is the standalone (non-Blitz) render path: it owns its texture allocation
//! and Vello renderer, and paints one page (paginated) or one band tile
//! (reflow) per call.

use std::time::Instant;

use appthere_canvas::texture::GpuTexture;
use appthere_canvas::{PageIndex, PageSource, RenderError};
use vello::{AaConfig, RenderParams, Scene};

use crate::doc_page_source::{A4_HEIGHT_PX, A4_WIDTH_PX, DocPageSource};

impl PageSource for DocPageSource {
    type Key = PageIndex;

    fn page_size_px(&self, index: PageIndex) -> (u32, u32) {
        let guard = self.layout_for_generation(self.current_generation());
        guard
            .as_ref()
            .and_then(|(_, layout)| layout.page_size_pts(index.0 as usize))
            .map(|(w, h)| (w.ceil() as u32, h.ceil() as u32))
            .unwrap_or((A4_WIDTH_PX, A4_HEIGHT_PX))
    }

    fn render(
        &self,
        index: PageIndex,
        scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<GpuTexture, RenderError> {
        let span = tracing::debug_span!("DocPageSource::render", index = index.0, scale = scale);
        let _enter = span.enter();
        let t_start = Instant::now();

        // Acquire layout and derive physical dimensions in one lock.
        let generation = self.current_generation();
        let layout_guard = self.layout_for_generation(generation);
        let Some((_, layout)) = layout_guard.as_ref() else {
            return Err(RenderError::Wgpu("layout unavailable".to_string()));
        };
        let Some((base_w, base_h)) = layout.page_size_pts(index.0 as usize) else {
            return Err(RenderError::Wgpu(format!("No such page: {}", index.0)));
        };
        let w = ((base_w * scale).ceil() as u32).max(1);
        let h = ((base_h * scale).ceil() as u32).max(1);

        let inner = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("doc-page-stub"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = inner.create_view(&wgpu::TextureViewDescriptor::default());

        let mut scene = Scene::new();
        let mut font_cache = self.font_cache.lock().unwrap_or_else(|e| e.into_inner());
        layout.paint_tile(
            &mut scene,
            &mut font_cache,
            index.0 as usize,
            scale,
            None,
            None,
            None,
        );
        drop(font_cache);
        drop(layout_guard);

        let mut guard = self.renderer.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            match crate::vello_init::create_vello_renderer(device) {
                Ok(r) => *guard = Some(r),
                Err(e) => return Err(RenderError::Wgpu(e.to_string())),
            }
        }
        let Some(renderer) = guard.as_mut() else {
            return Err(RenderError::Wgpu("renderer unavailable".to_string()));
        };

        let params = RenderParams {
            base_color: vello::peniko::Color::WHITE,
            width: w,
            height: h,
            antialiasing_method: AaConfig::Area,
        };
        renderer
            .render_to_texture(device, queue, &scene, &view, &params)
            .map_err(|e| RenderError::Wgpu(e.to_string()))?;

        tracing::debug!(
            index = index.0,
            scale = scale,
            width_px = w,
            height_px = h,
            elapsed_ms = t_start.elapsed().as_millis(),
            "page rendered",
        );

        Ok(GpuTexture {
            inner,
            width: w,
            height: h,
        })
    }
}
