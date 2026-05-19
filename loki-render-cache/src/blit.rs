// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Cached bilinear-blit pipeline for GPU texture downsampling.
//!
//! [`BlitPipeline`] builds the wgpu shader, bind-group layout, pipeline layout,
//! render pipeline, and sampler once in [`BlitPipeline::new`], then reuses them
//! across [`BlitPipeline::downsample`] calls.  This avoids the per-call pipeline
//! construction overhead that the [`crate::downsample_texture`] free function
//! previously incurred.
//!
//! **Device affinity**: `BlitPipeline` must be used with the same
//! `wgpu::Device` that was passed to [`BlitPipeline::new`].  Passing a
//! different device to `downsample` produces a wgpu validation error.

use crate::texture::{GpuTexture, allocate_texture};

/// Fullscreen-triangle blit shader (WGSL).
///
/// Samples the source texture with a linear sampler and writes to the
/// destination.  UV coordinates cover [0, 1]² exactly over the output.
const BLIT_WGSL: &str = "
struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VO {
    var xy = array<vec2<f32>, 3>(
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 2.0),
        vec2<f32>(2.0, 0.0),
    );
    var out: VO;
    out.pos = vec4<f32>(xy[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

@fragment
fn fs_main(in: VO) -> @location(0) vec4<f32> {
    return textureSample(t_src, s_src, in.uv);
}
";

/// Cached wgpu pipeline for bilinear texture downsampling.
///
/// Create once per device via [`BlitPipeline::new`] and reuse across multiple
/// [`BlitPipeline::downsample`] calls.
pub struct BlitPipeline {
    bind_group_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl BlitPipeline {
    /// Compiles the blit shader and builds all pipeline objects for `device`.
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit-pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit-rp"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            bind_group_layout,
            render_pipeline,
            sampler,
        }
    }

    /// Downsamples `src` into a new texture at `scale × src` dimensions.
    ///
    /// Uses the cached bilinear-filtered pipeline built in [`BlitPipeline::new`].
    /// `scale` must be in `(0.0, 1.0]`; values > 1.0 are clamped to 1.0 so
    /// this method never upsamples.
    #[must_use]
    pub fn downsample(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        src: &GpuTexture,
        scale: f32,
    ) -> GpuTexture {
        let scale = scale.clamp(f32::EPSILON, 1.0);
        let dst_w = ((src.width as f32 * scale).ceil() as u32).max(1);
        let dst_h = ((src.height as f32 * scale).ceil() as u32).max(1);
        let dst = allocate_texture(device, dst_w, dst_h, Some("blit-dst"));

        let src_view = src
            .inner
            .create_view(&wgpu::TextureViewDescriptor::default());
        let dst_view = dst
            .inner
            .create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
        dst
    }
}
