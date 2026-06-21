use anyrender::{WindowHandle, WindowRenderer};
use debug_timer::debug_timer;
use peniko::Color;
use rustc_hash::FxHashMap;
use std::sync::{
    Arc,
    atomic::{self, AtomicU64},
};
use vello::{
    AaConfig, AaSupport, RenderParams, Renderer as VelloRenderer, RendererOptions,
    Scene as VelloScene,
};
use wgpu::{Features, Limits, PresentMode, TextureFormat, TextureUsages};
use wgpu_context::{
    DeviceHandle, SurfaceRenderer, SurfaceRendererConfiguration, TextureConfiguration, WGPUContext,
};

use crate::{CustomPaintCtx, CustomPaintSource, DEFAULT_THREADS, VelloScenePainter};

static PAINT_SOURCE_ID: AtomicU64 = AtomicU64::new(0);

// Simple struct to hold the state of the renderer
struct ActiveRenderState {
    renderer: VelloRenderer,
    render_surface: SurfaceRenderer<'static>,
}

#[allow(clippy::large_enum_variant)]
enum RenderState {
    Active(ActiveRenderState),
    Suspended,
}

impl RenderState {
    fn current_device_handle(&self) -> Option<&DeviceHandle> {
        let RenderState::Active(state) = self else {
            return None;
        };
        Some(&state.render_surface.device_handle)
    }
}

#[derive(Clone)]
pub struct VelloRendererOptions {
    pub features: Option<Features>,
    pub limits: Option<Limits>,
    pub base_color: Color,
    pub antialiasing_method: AaConfig,
}

impl Default for VelloRendererOptions {
    fn default() -> Self {
        Self {
            features: None,
            limits: None,
            base_color: Color::WHITE,
            // COMPAT(android-mali): Mali drivers (Pixel 9 / Mali-G715 r54p2)
            // lose the Vulkan device executing Vello's MSAA16 fine-raster
            // pipeline on the first frame. Analytic area AA is Vello's
            // recommended mobile configuration and avoids the large
            // workgroup-memory MSAA variants entirely.
            #[cfg(target_os = "android")]
            antialiasing_method: AaConfig::Area,
            #[cfg(not(target_os = "android"))]
            antialiasing_method: AaConfig::Msaa16,
        }
    }
}

pub struct VelloWindowRenderer {
    // The fields MUST be in this order, so that the surface is dropped before the window
    // Window is cached even when suspended so that it can be reused when the app is resumed after being suspended
    render_state: RenderState,
    window_handle: Option<Arc<dyn WindowHandle>>,

    // Vello
    wgpu_context: WGPUContext,
    scene: VelloScene,
    config: VelloRendererOptions,

    custom_paint_sources: FxHashMap<u64, Box<dyn CustomPaintSource>>,
}
impl VelloWindowRenderer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::with_options(VelloRendererOptions::default())
    }

    pub fn with_options(config: VelloRendererOptions) -> Self {
        let features = config.features.unwrap_or_default()
            | Features::CLEAR_TEXTURE
            | Features::PIPELINE_CACHE;
        Self {
            wgpu_context: WGPUContext::with_features_and_limits(
                Some(features),
                config.limits.clone(),
            ),
            config,
            render_state: RenderState::Suspended,
            window_handle: None,
            scene: VelloScene::new(),
            custom_paint_sources: FxHashMap::default(),
        }
    }

    pub fn current_device_handle(&self) -> Option<&DeviceHandle> {
        self.render_state.current_device_handle()
    }

    pub fn register_custom_paint_source(&mut self, mut source: Box<dyn CustomPaintSource>) -> u64 {
        if let Some(device_handle) = self.render_state.current_device_handle() {
            source.resume(device_handle);
        }
        let id = PAINT_SOURCE_ID.fetch_add(1, atomic::Ordering::SeqCst);
        self.custom_paint_sources.insert(id, source);

        id
    }

    pub fn unregister_custom_paint_source(&mut self, id: u64) {
        if let Some(mut source) = self.custom_paint_sources.remove(&id) {
            // Give the source a chance to release any textures it registered
            // with the live renderer before it is dropped. The renderer retains
            // registered textures until `unregister_texture`; `suspend` cannot
            // call it (no ctx), so without this the source's last texture would
            // leak in the renderer's registry until the renderer is recreated.
            if let RenderState::Active(state) = &mut self.render_state {
                source.release(CustomPaintCtx::new(&mut state.renderer));
            }
            source.suspend();
            drop(source);
        }
    }
}

impl WindowRenderer for VelloWindowRenderer {
    type ScenePainter<'a>
        = VelloScenePainter<'a, 'a>
    where
        Self: 'a;

    fn is_active(&self) -> bool {
        matches!(self.render_state, RenderState::Active(_))
    }

    fn resume(&mut self, window_handle: Arc<dyn WindowHandle>, width: u32, height: u32) {
        // Create wgpu_context::SurfaceRenderer
        let render_surface = pollster::block_on(self.wgpu_context.create_surface(
            window_handle.clone(),
            SurfaceRendererConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                formats: vec![TextureFormat::Rgba8Unorm, TextureFormat::Bgra8Unorm],
                width,
                height,
                present_mode: PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
            Some(TextureConfiguration {
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            }),
        ))
        .expect("Error creating surface");

        // Create vello::Renderer
        let renderer = VelloRenderer::new(
            render_surface.device(),
            RendererOptions {
                // COMPAT(android-mali): compile only the area-AA pipeline
                // variants on Android — the MSAA variants fault the Mali
                // driver (see VelloRendererOptions::default).
                #[cfg(target_os = "android")]
                antialiasing_support: AaSupport::area_only(),
                #[cfg(not(target_os = "android"))]
                antialiasing_support: AaSupport::all(),
                // COMPAT(android-mali): Mali r54 drivers fault the device
                // executing Vello's compute dispatches (even area-only AA).
                // use_cpu runs the compute stages on the CPU; only fine
                // rasterization and the surface blit remain on the GPU.
                #[cfg(target_os = "android")]
                use_cpu: true,
                #[cfg(not(target_os = "android"))]
                use_cpu: false,
                num_init_threads: DEFAULT_THREADS,
                // TODO: add pipeline cache
                pipeline_cache: None,
            },
        )
        .unwrap();

        // Resume custom paint sources
        let device_handle = &render_surface.device_handle;
        for source in self.custom_paint_sources.values_mut() {
            source.resume(device_handle)
        }

        // Set state to Active
        self.window_handle = Some(window_handle);
        self.render_state = RenderState::Active(ActiveRenderState {
            renderer,
            render_surface,
        });
    }

    fn suspend(&mut self) {
        // Suspend custom paint sources
        for source in self.custom_paint_sources.values_mut() {
            source.suspend()
        }

        // Set state to Suspended
        self.render_state = RenderState::Suspended;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        if let RenderState::Active(state) = &mut self.render_state {
            state.render_surface.resize(width, height);
        };
    }

    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw_fn: F) {
        let RenderState::Active(state) = &mut self.render_state else {
            return;
        };

        let render_surface = &state.render_surface;

        debug_timer!(timer, feature = "log_frame_times");

        // Regenerate the vello scene
        draw_fn(&mut VelloScenePainter {
            inner: &mut self.scene,
            renderer: Some(&mut state.renderer),
            custom_paint_sources: Some(&mut self.custom_paint_sources),
        });
        timer.record_time("cmd");

        state
            .renderer
            .render_to_texture(
                render_surface.device(),
                render_surface.queue(),
                &self.scene,
                &render_surface.target_texture_view(),
                &RenderParams {
                    base_color: self.config.base_color,
                    width: render_surface.config.width,
                    height: render_surface.config.height,
                    antialiasing_method: self.config.antialiasing_method,
                },
            )
            .expect("failed to render to texture");
        timer.record_time("render");

        render_surface.maybe_blit_and_present();
        timer.record_time("present");

        render_surface.device().poll(wgpu::PollType::Wait).unwrap();

        timer.record_time("wait");
        timer.print_times("vello: ");

        // static COUNTER: AtomicU64 = AtomicU64::new(0);
        // println!("FRAME {}", COUNTER.fetch_add(1, atomic::Ordering::Relaxed));

        // Empty the Vello scene (memory optimisation)
        self.scene.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CustomPaintCtx, TextureHandle};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Records how many times each teardown hook fired, so a test can assert the
    /// unregister path drives source teardown.
    #[derive(Default)]
    struct Calls {
        suspend: AtomicUsize,
        release: AtomicUsize,
    }

    struct SpySource(Arc<Calls>);

    impl CustomPaintSource for SpySource {
        fn resume(&mut self, _device_handle: &DeviceHandle) {}
        fn suspend(&mut self) {
            self.0.suspend.fetch_add(1, Ordering::SeqCst);
        }
        fn render(
            &mut self,
            _ctx: CustomPaintCtx<'_>,
            _width: u32,
            _height: u32,
            _scale: f64,
        ) -> Option<TextureHandle> {
            None
        }
        fn release(&mut self, _ctx: CustomPaintCtx<'_>) {
            self.0.release.fetch_add(1, Ordering::SeqCst);
        }
    }

    // Constructing a `VelloWindowRenderer` only creates a wgpu `Instance` (no
    // adapter/device), so these run headlessly. The renderer starts `Suspended`;
    // the `release` (texture-unregister) path requires an `Active` renderer with
    // a real GPU surface and is verified on-device by watching RSS plateau while
    // scrolling a long document.

    #[test]
    fn unregister_tears_down_and_removes_the_source() {
        let mut renderer = VelloWindowRenderer::new();
        let calls = Arc::new(Calls::default());
        let id = renderer.register_custom_paint_source(Box::new(SpySource(calls.clone())));
        assert_eq!(renderer.custom_paint_sources.len(), 1);

        renderer.unregister_custom_paint_source(id);

        // The source is removed and suspended. (When suspended there is no live
        // renderer to unregister textures from, so `release` is correctly skipped;
        // the active-state release path is exercised on-device.)
        assert!(renderer.custom_paint_sources.is_empty());
        assert_eq!(calls.suspend.load(Ordering::SeqCst), 1, "suspend not called");
        assert_eq!(calls.release.load(Ordering::SeqCst), 0, "release on suspended");
    }

    #[test]
    fn unregister_unknown_id_is_a_noop() {
        let mut renderer = VelloWindowRenderer::new();
        renderer.unregister_custom_paint_source(123);
        assert!(renderer.custom_paint_sources.is_empty());
    }

    #[test]
    fn default_release_impl_is_a_noop() {
        // A source that registers no textures uses the default `release`, which
        // must compile and do nothing.
        struct Bare;
        impl CustomPaintSource for Bare {
            fn resume(&mut self, _d: &DeviceHandle) {}
            fn suspend(&mut self) {}
            fn render(
                &mut self,
                _c: CustomPaintCtx<'_>,
                _w: u32,
                _h: u32,
                _s: f64,
            ) -> Option<TextureHandle> {
                None
            }
        }
        let mut renderer = VelloWindowRenderer::new();
        let id = renderer.register_custom_paint_source(Box::new(Bare));
        renderer.unregister_custom_paint_source(id); // must not panic
        assert!(renderer.custom_paint_sources.is_empty());
    }
}
