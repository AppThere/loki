// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Offscreen visual conformance integration tests for Loki rendering backends.
//!
//! Scans `tests/conformance/documents/` for `.docx` and `.odt` files, renders
//! them using Vello to a wgpu offscreen texture, and compares the resulting
//! pixel data against reference PNGs.

use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::OnceLock;

use image::{Rgba, RgbaImage};
use loki_doc_model::io::DocumentImport;
use loki_layout::{
    layout_document, DocumentLayout, FontResources, LayoutMode, LayoutOptions,
};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_vello::{FontDataCache, paint_layout};
use wgpu::{Device, Queue};

// Global wgpu device/queue lock to reuse GPU context across tests.
static WGPU_STATE: OnceLock<(Device, Queue)> = OnceLock::new();

fn get_wgpu_state() -> &'static (Device, Queue) {
    WGPU_STATE.get_or_init(|| {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no wgpu adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .expect("failed to create wgpu device");

        (device, queue)
    })
}

// Compare two RGBA8 images with a pixel-diff threshold
fn compare_images(
    actual: &RgbaImage,
    reference: &RgbaImage,
    tolerance_pct: f32,
) -> Result<(), String> {
    if actual.dimensions() != reference.dimensions() {
        return Err(format!(
            "Dimension mismatch: actual={:?}, reference={:?}",
            actual.dimensions(),
            reference.dimensions()
        ));
    }

    let (w, h) = actual.dimensions();
    let total_pixels = w * h;
    let mut mismatched = 0;

    // Tolerance threshold for color differences to ignore minor anti-aliasing variations
    let color_threshold = 10i16;

    for y in 0..h {
        for x in 0..w {
            let act_px = actual.get_pixel(x, y);
            let ref_px = reference.get_pixel(x, y);

            let diff_r = (act_px[0] as i16 - ref_px[0] as i16).abs();
            let diff_g = (act_px[1] as i16 - ref_px[1] as i16).abs();
            let diff_b = (act_px[2] as i16 - ref_px[2] as i16).abs();
            let diff_a = (act_px[3] as i16 - ref_px[3] as i16).abs();

            if diff_r > color_threshold
                || diff_g > color_threshold
                || diff_b > color_threshold
                || diff_a > color_threshold
            {
                mismatched += 1;
            }
        }
    }

    let mismatch_pct = (mismatched as f32 / total_pixels as f32) * 100.0;
    if mismatch_pct > tolerance_pct {
        return Err(format!(
            "Mismatched pixels: {} / {} ({:.2}%, tolerance={:.2}%)",
            mismatched, total_pixels, mismatch_pct, tolerance_pct
        ));
    }

    Ok(())
}

// Generate diff image highlighting differences
fn create_diff_image(actual: &RgbaImage, reference: &RgbaImage) -> RgbaImage {
    let (w, h) = actual.dimensions();
    let mut diff_img = RgbaImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let act_px = actual.get_pixel(x, y);
            let ref_px = reference.get_pixel(x, y);

            let diff_r = (act_px[0] as i16 - ref_px[0] as i16).abs();
            let diff_g = (act_px[1] as i16 - ref_px[1] as i16).abs();
            let diff_b = (act_px[2] as i16 - ref_px[2] as i16).abs();
            let diff_a = (act_px[3] as i16 - ref_px[3] as i16).abs();

            if diff_r > 10 || diff_g > 10 || diff_b > 10 || diff_a > 10 {
                // Mismatched pixel: paint red
                diff_img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            } else {
                // Matched pixel: paint dimmed actual image
                diff_img.put_pixel(
                    x,
                    y,
                    Rgba([
                        (act_px[0] / 3) + 170,
                        (act_px[1] / 3) + 170,
                        (act_px[2] / 3) + 170,
                        act_px[3],
                    ]),
                );
            }
        }
    }

    diff_img
}

fn render_page(
    layout: &DocumentLayout,
    page_index: usize,
    _font_resources: &mut FontResources,
) -> Option<RgbaImage> {
    let (canvas_width, canvas_height) = match &layout {
        DocumentLayout::Paginated(pl) => {
            if page_index >= pl.pages.len() {
                return None;
            }
            (
                (pl.page_size.width + 32.0) as u32,
                (pl.page_size.height + 32.0) as u32,
            )
        }
        _ => (
            (layout.content_width() + 32.0) as u32,
            (layout.total_height() + 32.0) as u32,
        ),
    };

    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    paint_layout(
        &mut scene,
        &layout,
        &mut font_cache,
        (16.0, 16.0),
        1.0,
        Some(page_index),
    );

    let (device, queue) = get_wgpu_state();

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
        let mut data = Vec::with_capacity((unpadded_bytes_per_row * canvas_height) as usize);
        for row in 0..canvas_height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            data.extend_from_slice(&mapped_range[start..end]);
        }
        data
    };
    readback_buffer.unmap();

    let img = RgbaImage::from_raw(canvas_width, canvas_height, pixel_data)
        .expect("failed to construct RgbaImage from raw pixel buffer");
    Some(img)
}

#[test]
fn test_visual_conformance() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let conformance_dir = manifest_dir.join("tests").join("conformance");
    let docs_dir = conformance_dir.join("documents");
    let refs_dir = conformance_dir.join("references");
    let outs_dir = conformance_dir.join("outputs");

    // Ensure directories exist
    fs::create_dir_all(&docs_dir).unwrap();
    fs::create_dir_all(&refs_dir).unwrap();
    fs::create_dir_all(&outs_dir).unwrap();

    let generate_references = std::env::var("GENERATE_REFERENCES").is_ok();

    // Scan for DOCX and ODT files
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&docs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "docx" || ext == "odt" {
                    files.push(path);
                }
            }
        }
    }

    if files.is_empty() {
        println!("No test files found in {:?}", docs_dir);
        return;
    }

    let mut font_resources = FontResources::new();
    let mut failed = false;
    let mut failure_messages = Vec::new();

    for file_path in files {
        let file_name = file_path.file_name().unwrap().to_str().unwrap();
        let stem = file_path.file_stem().unwrap().to_str().unwrap();
        let extension = file_path.extension().unwrap().to_str().unwrap();

        println!("Running conformance test for {}...", file_name);

        let file = fs::File::open(&file_path).unwrap();
        let document = match extension {
            "docx" => DocxImport::import(file, DocxImportOptions::default())
                .map_err(|e| format!("DOCX Import Error: {:?}", e)),
            "odt" => OdtImport::import(file, OdtImportOptions::default())
                .map_err(|e| format!("ODT Import Error: {:?}", e)),
            _ => continue,
        };

        let document = match document {
            Ok(doc) => doc,
            Err(e) => {
                failed = true;
                failure_messages.push(format!("File {}: {}", file_name, e));
                continue;
            }
        };

        let layout = layout_document(
            &mut font_resources,
            &document,
            LayoutMode::Paginated,
            1.0,
            &LayoutOptions::default(),
        );

        let page_count = match &layout {
            DocumentLayout::Paginated(pl) => pl.pages.len(),
            _ => 1,
        };

        for page_idx in 0..page_count {
            let rendered_img = render_page(&layout, page_idx, &mut font_resources)
                .expect("failed to render page");

            let ref_filename = format!("{}_page{}.png", stem, page_idx);
            let ref_path = refs_dir.join(&ref_filename);

            if generate_references {
                rendered_img.save(&ref_path).unwrap();
                println!("Generated reference image: {:?}", ref_path);
            } else {
                if !ref_path.exists() {
                    failed = true;
                    let msg = format!("Missing reference image: {:?}", ref_path);
                    println!("{}", msg);
                    failure_messages.push(msg);
                    continue;
                }

                let ref_img = image::open(&ref_path).unwrap().to_rgba8();

                match compare_images(&rendered_img, &ref_img, 0.5) {
                    Ok(_) => {
                        println!("Page {} passed.", page_idx);
                    }
                    Err(e) => {
                        failed = true;
                        let msg = format!("File {} Page {} failed: {}", file_name, page_idx, e);
                        println!("{}", msg);
                        failure_messages.push(msg);

                        // Save actual and diff output
                        let act_path =
                            outs_dir.join(format!("{}_page{}_actual.png", stem, page_idx));
                        let diff_path =
                            outs_dir.join(format!("{}_page{}_diff.png", stem, page_idx));

                        rendered_img.save(&act_path).unwrap();
                        let diff_img = create_diff_image(&rendered_img, &ref_img);
                        diff_img.save(&diff_path).unwrap();

                        println!("  Saved actual: {:?}", act_path);
                        println!("  Saved diff:   {:?}", diff_path);
                    }
                }
            }
        }
    }

    if failed {
        panic!(
            "Visual conformance tests failed:\n{}",
            failure_messages.join("\n")
        );
    }
}
