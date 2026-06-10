# Android GPU (Vello) instant-crash investigation — 2026-06-10

## Question

Loki crashed instantly on a Pixel 9 when using the GPU-accelerated Vello
renderer. A prior analysis blamed a defective GPU driver. Godot 4 runs
Vulkan on the same device without issue. Can the GPU renderer be used on
all targets?

## Verdict

**The "defective GPU driver" theory is not supported by the evidence.**
The Pixel 9 (Tensor G4, Mali-G715, Vulkan 1.3) is a mainstream wgpu/Vello
target — Vello's own `with_winit` example ships Android support, and the
device running Godot's Vulkan renderer confirms basic driver health. The
far more likely cause is a **Rust panic during renderer initialisation
that was invisible** because no panic hook routes panic messages to
logcat — on Android, the default panic handler writes to stderr, which
the OS discards. An init panic and a native driver crash are
indistinguishable without a tombstone.

Critically, at least **three instant-crash-at-launch Android bugs existed
in this codebase at the time the GPU path was likely tested, and all
three have since been fixed** (for the CPU path):

### Candidate cause 1 — zero-size wgpu surface at `resumed()` (fixed in c1fbabf)

`patches/blitz-shell/src/window.rs:198-209` now documents exactly this:
on Android, `winit_window.inner_size()` reads the ANativeWindow buffer
dimensions, which are **0×0 until the first `WindowResized` event** —
the window object exists after InitWindow/Resumed but its buffer is
sized later. Calling `renderer.resume()` then makes
`anyrender_vello` configure a wgpu surface of width/height 0, which
panics instantly inside `Surface::configure` validation. The current
patch defers renderer activation to the `Resized` event; a GPU build
tested **before** this fix would crash at launch, every time, on every
device — precisely matching the reported symptom.

### Candidate cause 2 — Android 16 double `ANativeActivity_onCreate` (fixed in 976e2be)

On Android 16 (which the Pixel 9 runs), `ANativeActivity_onCreate` fires
twice in rapid succession, spawning two concurrent `android_main`
threads. Before 976e2be, `blitz_shell::ANDROID_APP` was a `OnceLock`
(double-set panic) and there was no re-entry guard in
`loki-text/src/lib.rs::android_main` — two concurrent event loops fight
over one window. This crash is renderer-independent, so a GPU test build
predating 976e2be would have hit it regardless of Vello.

### Candidate cause 3 — hard panics in the GPU init path (still present)

`anyrender_vello-0.6.2/src/window_renderer.rs` (crates.io, not patched):

- `resume()` line 148: `.expect("Error creating surface")` — fails if
  surface creation, adapter selection, or `request_device` fails.
- line 161: `VelloRenderer::new(...).unwrap()` — fails if Vello's
  compute pipelines can't be created.
- `wgpu_context-0.1.2` requests **`Limits::default()`** (full
  WebGPU-tier limits) when no override is supplied — it does *not*
  clamp to `adapter.limits()`. Loki launches via plain
  `dioxus::launch(App)` with no `Limits` config, so any single default
  limit the Mali driver doesn't meet fails `request_device` → panic.

Any of these panics is invisible without a logcat panic hook.

### Candidate cause 4 — genuine Mali driver issue (least likely)

Mali drivers have real history (Pixel 6 Mali r46 driver bugs, fixed in
r47; Godot has open Mali-G715 Vulkan crash reports), and Vello stresses
compute paths Godot doesn't. But driver-level failures usually present
as artifacts or device-lost during rendering, not a deterministic
instant crash at launch. Treat this as the fallback hypothesis only
after 1–3 are excluded with a real backtrace.

## Recommended plan

1. **Install a panic→logcat hook first** (one-liner with the
   `log_panics` crate, or `std::panic::set_hook` → `log::error!`) in
   `android_main` before anything else. Without this, no Android crash
   is diagnosable.
2. **Retest the GPU build on current main**: `build-android.ps1 -Gpu`,
   then `adb logcat -s LOKI RustStdoutStderr AndroidRuntime DEBUG`.
   There is a good chance it simply works now — the zero-size-surface
   and double-onCreate fixes both post-date the original GPU attempt.
   Note: first GPU launch compiles all Vello compute shaders (~1–2 s,
   known upstream issue) — don't mistake a slow first frame for a hang.
3. **If it still crashes**, the logcat panic message (or
   `adb bugreport` tombstone) will identify which of the three init
   points fails. If it's `request_device`, pass conservative `Limits`
   via the launch config (the dioxus-native patch already downcasts a
   `Limits` config object from `LaunchBuilder::with_cfg`).
4. **Longer term — runtime fallback instead of compile-time cfg**: the
   current `--cfg android_gpu` split means separate APKs and a CPU
   renderer maintained in parallel, which the project wants to avoid.
   Instead, probe wgpu at startup (request adapter + device with
   Vello's requirements); on success use `VelloWindowRenderer`,
   otherwise fall back to `VelloCpuWindowRenderer`. Both implement
   `WindowRenderer`, so an enum wrapper in the dioxus-native patch
   makes the choice dynamic — one APK, GPU-first everywhere, CPU only
   where the device genuinely can't (emulator/SwiftShader).

## References

- wgpu adapter-failure crash on Android: https://github.com/gfx-rs/wgpu/issues/2419
- Vello Android support (`with_winit` via cargo-apk): https://github.com/linebender/vello
- Vello shader-compile startup cost / pipeline caching: https://github.com/gfx-rs/wgpu/issues/5293
- Godot Mali-G715 (Pixel 8 Pro) Vulkan crash report: https://github.com/godotengine/godot/issues/115442
- Blitz / dioxus-native architecture: https://github.com/DioxusLabs/blitz
