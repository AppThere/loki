// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Render a document layout to a PNG file via the full loki-vello pipeline.
//!
//! This example constructs a minimal [`loki_layout::DocumentLayout`] containing
//! a few coloured rectangles, paints it into a Vello scene, renders the scene
//! via wgpu to an off-screen texture, reads back the pixels, and saves them as
//! a PNG.
//!
//! # Usage
//!
//! ```text
//! cargo run --example render_to_png
//! cargo run --example render_to_png -- output.png
//! ```
//!
//! When a real document importer and layouter are available the two `// TODO`
//! blocks below can be replaced with actual import + layout calls.

use std::num::NonZeroUsize;

use loki_doc_model::io::DocumentImport;
use loki_layout::{
    ContinuousLayout, DocumentLayout, LayoutColor, LayoutRect, PositionedItem, PositionedRect,
    layout_document, LayoutMode, FontResources,
};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_vello::{FontDataCache, paint_layout};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // Handle arguments: render_to_png [input.docx] [output.png]
    let (input_path, output_path) = match args.len() {
        1 => (None, "output.png"),
        2 => (None, args[1].as_str()),
        _ => (Some(args[1].as_str()), args[2].as_str()),
    };

    let mut font_resources = FontResources::new();

    // ── 1. Build layout ───────────────────────────────────────────────────────
    let t0 = std::time::Instant::now();
    let layout = if let Some(input) = input_path {
        println!("Reading {}...", input);
        let file = std::fs::File::open(input).expect("could not open input file");
        let document = DocxImport::import(file, DocxImportOptions::default())
            .expect("DOCX import failed");
        
        layout_document(&mut font_resources, &document, LayoutMode::Pageless, 1.0)
    } else {
        // Fallback to a minimal layout if no input file is provided
        DocumentLayout::Continuous(ContinuousLayout {
            content_width: 400.0,
            total_height: 300.0,
            items: vec![
                // Background
                PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(0.0, 0.0, 400.0, 300.0),
                    color: LayoutColor::WHITE,
                }),
                // Red block
                PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(40.0, 40.0, 120.0, 80.0),
                    color: LayoutColor { r: 0.9, g: 0.2, b: 0.2, a: 1.0 },
                }),
                // Green block
                PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(200.0, 40.0, 120.0, 80.0),
                    color: LayoutColor { r: 0.2, g: 0.8, b: 0.3, a: 1.0 },
                }),
                // Blue block
                PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(40.0, 160.0, 280.0, 80.0),
                    color: LayoutColor { r: 0.2, g: 0.4, b: 0.9, a: 1.0 },
                }),
            ],
        })
    };

    let t_layout = t0.elapsed();

    let canvas_width = (layout.content_width() + 32.0) as u32;
    let canvas_height = (layout.total_height() + 32.0) as u32;

    // ── 2. Build Vello scene ──────────────────────────────────────────────────
    let t1 = std::time::Instant::now();
    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    paint_layout(&mut scene, &layout, &mut font_cache, (16.0, 16.0), 1.0);
    let t_scene = t1.elapsed();

    // ── 3. Set up wgpu ────────────────────────────────────────────────────────
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no wgpu adapter found — a software rasterizer (e.g. llvmpipe) is required");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
    ))
    .expect("failed to create wgpu device");

    // ── 4. Create render-target texture ──────────────────────────────────────
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("loki_render_target"),
        size: wgpu::Extent3d {
            width: canvas_width,
            height: canvas_height,
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
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // ── 5. Render ─────────────────────────────────────────────────────────────
    let mut renderer = vello::Renderer::new(
        &device,
        vello::RendererOptions {
            antialiasing_support: vello::AaSupport::area_only(),
            num_init_threads: NonZeroUsize::new(1),
            ..Default::default()
        },
    )
    .expect("vello renderer initialisation failed");

    renderer
        .render_to_texture(
            &device,
            &queue,
            &scene,
            &texture_view,
            &vello::RenderParams {
                base_color: peniko::Color::new([1.0, 1.0, 1.0, 1.0]),
                width: canvas_width,
                height: canvas_height,
                antialiasing_method: vello::AaConfig::Area,
            },
        )
        .expect("vello render failed");
    let t_render = t1.elapsed() - t_scene;

    // ── 6. Read back from GPU ─────────────────────────────────────────────────
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded_bytes_per_row = canvas_width * 4;
    let padding = (align - unpadded_bytes_per_row % align) % align;
    let padded_bytes_per_row = unpadded_bytes_per_row + padding;
    let buffer_size = (padded_bytes_per_row * canvas_height) as u64;

    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(canvas_height),
            },
        },
        wgpu::Extent3d {
            width: canvas_width,
            height: canvas_height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let pixel_data = {
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::Wait).expect("GPU poll failed");
        
        let mapped_range = slice.get_mapped_range();
        
        // Strip padding
        let mut data = Vec::with_capacity((unpadded_bytes_per_row * canvas_height) as usize);
        for row in 0..canvas_height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            data.extend_from_slice(&mapped_range[start..end]);
        }
        data
    };
    readback_buffer.unmap();

    // ── 7. Save PNG ───────────────────────────────────────────────────────────
    image::save_buffer(
        output_path,
        &pixel_data,
        canvas_width,
        canvas_height,
        image::ColorType::Rgba8,
    )
    .expect("PNG save failed");

    println!("Rendered {canvas_width}×{canvas_height} → {output_path}");
    println!("  layout: {:?}  scene: {:?}  gpu render+readback: {:?}",
        t_layout, t_scene, t_render);
}
