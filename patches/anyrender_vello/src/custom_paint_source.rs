use peniko::ImageData;
use vello::Renderer as VelloRenderer;
use wgpu::Texture;
pub use wgpu_context::DeviceHandle;

pub trait CustomPaintSource: 'static {
    fn resume(&mut self, device_handle: &DeviceHandle);
    fn suspend(&mut self);
    fn render(
        &mut self,
        ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle>;

    /// Called when the source is being unregistered while the renderer is still
    /// active, giving it a [`CustomPaintCtx`] so it can `unregister_texture` any
    /// texture it registered. Textures handed to the renderer via
    /// [`CustomPaintCtx::register_texture`] live in the renderer's registry until
    /// explicitly unregistered; `suspend` cannot do this (it has no ctx), so
    /// without this hook a source's last texture would leak when the source is
    /// dropped (e.g. a virtualized tile scrolling out of view). The default is a
    /// no-op for sources that register no textures.
    fn release(&mut self, _ctx: CustomPaintCtx<'_>) {}
}

pub struct CustomPaintCtx<'r> {
    pub(crate) renderer: &'r mut VelloRenderer,
}

#[derive(Clone, PartialEq)]
pub struct TextureHandle(pub ImageData);

impl CustomPaintCtx<'_> {
    pub(crate) fn new<'a>(renderer: &'a mut VelloRenderer) -> CustomPaintCtx<'a> {
        CustomPaintCtx { renderer }
    }

    pub fn register_texture(&mut self, texture: Texture) -> TextureHandle {
        TextureHandle(self.renderer.register_texture(texture))
    }

    pub fn unregister_texture(&mut self, handle: TextureHandle) {
        self.renderer.unregister_texture(handle.0);
    }

    /// SPIKE(mem): the window's `vello::Renderer`, so a custom paint source can
    /// render its own scenes on it rather than constructing a second one.
    ///
    /// Each `vello::Renderer` allocates ~165 MiB of fixed-size scratch buffers
    /// (`vello_encoding::BufferSizes::new` — hand-picked for the `paris-30k`
    /// stress scene, independent of the actual scene), so a second instance is
    /// a flat 165 MiB cost regardless of what it draws.
    ///
    /// Sound at this point in the frame: `VelloWindowRenderer::render` invokes
    /// custom sources from inside `draw_fn`, which completes *before* the
    /// window's own `render_to_texture`. There is no open render pass, and
    /// `render_to_texture` is a self-contained encode-and-submit, so a tile
    /// render here is sequenced before the window render rather than nested in
    /// it.
    pub fn renderer_mut(&mut self) -> &mut VelloRenderer {
        self.renderer
    }
}
