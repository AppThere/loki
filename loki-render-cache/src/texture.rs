// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! GPU texture allocation and downsampling blit.
//!
//! Only compiled when the `gpu` feature is active.



/// Fullscreen-triangle blit shader (WGSL).
///
/// Samples the source texture with a linear sampler and writes to the
/// destination. UV coordinates cover [0, 1]² exactly over the output.
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

/// A GPU texture together with its pixel dimensions.
///
/// Wraps [`wgpu::Texture`] and records `width`/`height` so byte-budget
/// calculations (see [`crate::page_cache::PageCache::cold_bytes`]) don't
/// need a separate GPU query.
#[derive(Debug)]
pub struct GpuTexture {
    /// The underlying wgpu texture object.
    pub inner: wgpu::Texture,
    /// Width of the texture in pixels.
    pub width: u32,
    /// Height of the texture in pixels.
    pub height: u32,
}

impl GpuTexture {
    /// Returns the approximate GPU memory footprint in bytes (RGBA8 = 4 bytes
    /// per pixel, no mip-maps).
    #[must_use]
    pub fn byte_size(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4
    }
}

/// Allocates a blank RGBA8 texture suitable for both rendering and sampling.
///
/// The texture has [`wgpu::TextureUsages::RENDER_ATTACHMENT`],
/// [`wgpu::TextureUsages::TEXTURE_BINDING`], and
/// [`wgpu::TextureUsages::COPY_SRC`].
#[must_use]
pub fn allocate_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: Option<&str>,
) -> GpuTexture {
    let inner = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    GpuTexture { inner, width, height }
}

/// Downsamples `src` into a new texture at `scale` × `src` dimensions.
///
/// Uses a bilinear-filtered fullscreen-triangle blit via a wgpu render pass.
/// `scale` must be in `(0.0, 1.0]`; values > 1.0 are clamped to 1.0 so this
/// function never upsamples.
#[must_use]
pub fn downsample_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src: &GpuTexture,
    scale: f32,
) -> GpuTexture {
    let scale = scale.clamp(f32::EPSILON, 1.0);
    let dst_w = ((src.width as f32 * scale).ceil() as u32).max(1);
    let dst_h = ((src.height as f32 * scale).ceil() as u32).max(1);
    let dst = allocate_texture(device, dst_w, dst_h, Some("blit-dst"));

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blit"),
        source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("blit-pl"),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit-rp"),
        layout: Some(&pl),
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
    let src_view = src.inner.create_view(&wgpu::TextureViewDescriptor::default());
    let dst_view = dst.inner.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit-bg"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
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
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    dst
}
