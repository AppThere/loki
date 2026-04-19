# Blitz / Dioxus Native — Vello Scene Integration Research

**Date:** 2026-04-19  
**Scope:** How to submit a `vello::Scene` built from a `loki_doc_model::Document` into
Blitz's render loop, replacing the current placeholder `div` in `WgpuSurface`.

---

## 1. What Blitz Does Internally (render pipeline)

```
Dioxus component tree
        │  RSX → virtual DOM diff
        ▼
blitz-dom  (layout engine — taffy/CSS)
        │  SpecialElementData::Canvas { custom_paint_source_id }
        ▼
blitz-paint  (traverses laid-out DOM nodes)
        │  calls VelloWindowRenderer::render()
        ▼
anyrender_vello  VelloWindowRenderer
        │  for each Canvas node: CustomPaintSource::render(ctx, w, h, scale)
        │  caller provides ctx.register_texture(wgpu::Texture) → TextureHandle
        │  texture is composited into the Vello background scene
        ▼
vello::Renderer::render_to_surface  (Blitz's own renderer — internal)
        │
        ▼
wgpu::Surface  (the window)
```

Key crates and versions (from `Cargo.lock`):

| Crate | Version | Role |
|---|---|---|
| `dioxus-native` | 0.7.4 | Dioxus renderer; exposes `use_wgpu` hook |
| `anyrender_vello` | 0.6.2 | `VelloWindowRenderer`; `CustomPaintSource` trait |
| `blitz-dom` | 0.2.4 | DOM/layout; canvas element trigger |
| `wgpu_context` | 0.1.2 | `DeviceHandle` (device + queue) |
| `vello` | 0.6.x | Scene builder + renderer |

---

## 2. Available Integration Points

### 2a. `CustomPaintSource` trait  
**File:** `anyrender_vello-0.6.2/src/custom_paint_source.rs`

```rust
pub trait CustomPaintSource: 'static {
    /// Called when a wgpu device becomes available (window created / resumed).
    /// Store `device_handle.device` and `device_handle.queue` here.
    fn resume(&mut self, device_handle: &DeviceHandle);

    /// Called when the window is hidden or destroyed.
    fn suspend(&mut self);

    /// Called each frame for every `<canvas src="{id}">` node in the DOM.
    /// `width`/`height` are the CSS-layout pixel dimensions of the canvas element.
    /// Return `Some(TextureHandle)` to composite the texture, or `None` to skip.
    fn render(
        &mut self,
        ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle>;
}
```

`CustomPaintCtx` exposes:

```rust
pub struct CustomPaintCtx<'r> {
    pub(crate) renderer: &'r mut VelloRenderer,  // NOT accessible outside anyrender_vello
}

impl CustomPaintCtx<'_> {
    /// Hand a wgpu texture to Blitz for compositing.
    pub fn register_texture(&mut self, texture: wgpu::Texture) -> TextureHandle { … }

    /// Release a previously registered texture.
    pub fn unregister_texture(&mut self, handle: TextureHandle) { … }
}
```

`DeviceHandle` (from `wgpu_context-0.1.2`):

```rust
pub struct DeviceHandle {
    pub device:   wgpu::Device,
    pub queue:    wgpu::Queue,
    pub adapter:  wgpu::Adapter,
    pub instance: wgpu::Instance,
}
```

### 2b. `use_wgpu` Dioxus hook  
**File:** `dioxus-native-0.7.4/src/dioxus_renderer.rs`  
**Re-exported as:** `dioxus::native::use_wgpu`

```rust
pub fn use_wgpu<T: CustomPaintSource>(create_source: impl FnOnce() -> T) -> u64
```

Registers a `CustomPaintSource` with the renderer and returns a stable `u64` source ID.
The source is unregistered automatically when the component unmounts.

### 2c. `<canvas>` DOM trigger  
**File:** `blitz-dom-0.2.4/src/mutator.rs`

When blitz-dom encounters `<canvas src="42">` (where `"42"` is the source ID returned by
`use_wgpu`), it parses the integer and sets:

```rust
SpecialElementData::Canvas { custom_paint_source_id: 42 }
```

blitz-paint then invokes `CustomPaintSource::render()` for that node each frame.

---

## 3. Recommended Approach — C: Render to wgpu Texture via `CustomPaintSource`

### Why this approach

| Approach | Description | Verdict |
|---|---|---|
| A: Patch Blitz internals | Reach into `VelloWindowRenderer` to add scenes | ❌ Requires forking Blitz |
| B: Render to PNG in memory, use `<img>` | CPU round-trip every frame | ❌ Far too slow |
| **C: `CustomPaintSource` + `ctx.register_texture`** | **Officially supported extension point** | ✅ Correct path |
| D: Separate window / offscreen surface | Cannot composite with Dioxus UI | ❌ No UI integration |

### Implementation sketch

```rust
// loki-text/src/components/wgpu_surface.rs

use dioxus::native::use_wgpu;
use anyrender_vello::{CustomPaintSource, CustomPaintCtx, DeviceHandle, TextureHandle};

struct LokiDocumentSource {
    // Shared with the Dioxus component via Arc<Mutex<>>
    document: Arc<Mutex<Option<Document>>>,
    // Populated in resume()
    device:   Option<wgpu::Device>,
    queue:    Option<wgpu::Queue>,
    renderer: Option<vello::Renderer>,
    // Texture persisted across frames for reuse
    texture:  Option<(wgpu::Texture, TextureHandle)>,
}

impl CustomPaintSource for LokiDocumentSource {
    fn resume(&mut self, dh: &DeviceHandle) {
        let renderer = vello::Renderer::new(
            &dh.device,
            RendererOptions { surface_format: None, use_cpu: false, antialiasing_support: AaSupport::all(), num_init_threads: NonZeroUsize::new(1) },
        ).expect("vello renderer");
        self.device   = Some(dh.device.clone());
        self.queue    = Some(dh.queue.clone());
        self.renderer = Some(renderer);
    }

    fn suspend(&mut self) {
        self.device   = None;
        self.queue    = None;
        self.renderer = None;
        self.texture  = None;
    }

    fn render(&mut self, mut ctx: CustomPaintCtx<'_>, width: u32, height: u32, scale: f64) -> Option<TextureHandle> {
        let (device, queue, renderer) =
            (self.device.as_ref()?, self.queue.as_ref()?, self.renderer.as_mut()?);

        // Build vello scene from current document snapshot.
        let mut scene = Scene::new();
        if let Ok(guard) = self.document.lock() {
            if let Some(doc) = guard.as_ref() {
                let mut font_resources = FontResources::new();  // ideally cached
                let layout = layout_document(&mut font_resources, doc, LayoutMode::Pageless, scale as f32);
                let mut font_cache = FontDataCache::new();
                paint_layout(&mut scene, &layout, &mut font_cache, (0.0, 0.0), scale as f32);
            }
        }

        // Render scene to a wgpu texture.
        let texture = device.create_texture(&wgpu::TextureDescriptor { … });
        let params = vello::RenderParams { base_color: Color::WHITE, width, height, antialiasing_method: AaConfig::Msaa16 };
        renderer.render_to_texture(device, queue, &scene, &texture.create_view(&Default::default()), &params).ok()?;

        // Hand texture to Blitz.
        let handle = ctx.register_texture(texture);
        Some(handle)
    }
}

// In WgpuSurface component:
#[allow(non_snake_case)]
pub fn WgpuSurface(props: WgpuSurfaceProps) -> Element {
    let doc_state: Arc<Mutex<Option<Document>>> = use_hook(|| Arc::new(Mutex::new(None)));

    // Update shared document when props change.
    *doc_state.lock().unwrap() = props.document.clone();

    let canvas_id = use_wgpu(|| LokiDocumentSource {
        document: Arc::clone(&doc_state),
        device: None, queue: None, renderer: None, texture: None,
    });

    rsx! {
        canvas {
            src: "{canvas_id}",
            style: "width: {PAGE_WIDTH_PX}px; height: {PAGE_HEIGHT_PX}px;",
        }
    }
}
```

---

## 4. Effort Estimate

| Task | Complexity | Est. time |
|---|---|---|
| Define `LokiDocumentSource` struct + `CustomPaintSource` impl | Medium | 2–3 h |
| Wire `FontResources` caching inside the source (not per-frame) | Medium | 1 h |
| Texture format negotiation (BGRA8 vs RGBA8) | Low | 30 min |
| Replace placeholder `div` with `<canvas src="{id}">` | Trivial | 15 min |
| End-to-end smoke test with a real DOCX | Low | 30 min |
| **Total** | | **~4–5 h** |

---

## 5. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **`anyrender_vello` version coupling** — `CustomPaintSource` is in an `0.6.x` crate with no stability guarantee; a Blitz update may rename or move the trait. | High | Pin exact version in `Cargo.lock`; write an integration test that fails fast if the trait disappears. |
| **Texture format mismatch** — Blitz's compositor expects a specific `wgpu::TextureFormat` (likely `Bgra8UnormSrgb`). Providing the wrong format silently composites garbage pixels. | Medium | Inspect `register_texture` source for format validation; create texture with `TextureFormat::Bgra8UnormSrgb` initially. |
| **Two-renderer overhead** — `LokiDocumentSource` creates its own `vello::Renderer` alongside Blitz's internal renderer. Both allocate GPU pipelines, doubling shader compile time at startup. | Medium | One renderer per `LokiDocumentSource` instance; ensure only one instance is registered at a time. |
| **`FontResources` per-frame allocation** — creating a new `FontResources` each `render()` call triggers system-font re-discovery, causing jank. | High | Store `FontResources` inside `LokiDocumentSource`; initialize in `resume()`, persist across calls. |
| **`ctx.renderer` is `pub(crate)`** — if future work requires calling internal Vello APIs (e.g. image upload), this is inaccessible. | Low | Covered by `ctx.register_texture`; unlikely to need direct renderer access. |
| **Document clone on every frame** — if `Document` is large, locking and cloning it each render is expensive. | Medium | Store a pre-built `DocumentLayout` inside the source; invalidate only on document change via a generation counter. |
