# Workspace Dependency Patches

This file documents every `[patch]` entry in the root `Cargo.toml`.
Update this file whenever a patch is added, modified, or removed.

## Why we patch

Loki targets Dioxus Native 0.7 and Blitz, both of which are pre-1.0 crates
with evolving APIs. Patches are the correct Rust mechanism for working around
upstream gaps while those gaps are being resolved. Each patch below is
temporary and has a documented removal condition.

---

## Active patches

### fontique — 0.8.0

**Source:** `patches/fontique/` (local), vendored from upstream commit
`8dbecc0545a0c97eb605937b928bc186d2d1295c` in
[linebender/parley](https://github.com/linebender/parley) (`fontique/` path
in that monorepo).

**Fixes:** Two related issues with the crates.io publication of fontique 0.8.0:

1. **Missing package alias.** The crates.io publication lost the
   `fontconfig_sys = { package = "yeslogic-fontconfig-sys", ... }` alias
   during Cargo normalization. The source in
   `src/backend/fontconfig.rs` uses `use fontconfig_sys::…` and requires
   this alias to compile.

2. **Workspace feature-unification conflict.** `blitz-dom` depends on
   fontique 0.6.0 and activates `yeslogic-fontconfig-sys/dlopen` for the
   entire build graph. Without the patch, fontique 0.8.0 is built without
   dlopen and fails on static-C imports because the two versions of the
   yeslogic-fontconfig-sys feature cannot agree. The patch enables
   `fontconfig-dlopen` on 0.8.0 so both versions use the same linkage mode.

**Root cause:** Bug in the fontique 0.8.0 crates.io publish pipeline (package
alias dropped during Cargo manifest normalisation). Compounded by Cargo
feature-unification behaviour when two semver-incompatible versions of
fontique coexist in the dependency graph.

**Upstream status:** No upstream issue filed as of 2026-05-03. Upstream
repository is [linebender/parley](https://github.com/linebender/parley).

**Removal condition:** Remove when a post-0.8.0 fontique release on crates.io
restores the `fontconfig_sys` package alias and the dlopen/static linkage
conflict is resolved (either fontique 0.8 is no longer paired with blitz-dom's
fontique 0.6, or upstream aligns feature flags).

**Added:** 2026-04-13 (introduced in the loki-text scaffold commit).

---

### dioxus-native-dom — 0.7.4

**Source:** `patches/dioxus-native-dom/` (local), vendored from upstream
commit `1eb00b5e0080ab4bd6a11ddd0a01c97f28493e04` in
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus)
(`packages/native-dom/` path). The vendor copy carries local modifications
(`dirty: true` in `.cargo_vcs_info.json`).

**Fixes:** The upstream dioxus-native-dom 0.7.4 panics at runtime for any
event type whose `HtmlEventConverter` implementation is a placeholder
`unimplemented!()`. The affected methods include:

- `convert_composition_data` — called for IME (CJK/RTL) input
- `convert_touch_data` — called for all touch events
- `convert_pointer_data`, `convert_scroll_data`, `convert_wheel_data`
- `convert_cancel_data`, `convert_clipboard_data`, `convert_drag_data`,
  `convert_image_data`, `convert_media_data`, `convert_mounted_data`,
  `convert_animation_data`, `convert_selection_data`, `convert_toggle_data`,
  `convert_transition_data`, `convert_resize_data`, `convert_visible_data`

**`onmounted` / `MountedData` (PATCH(loki), 2026-06-11).** `convert_mounted_data`
is implemented, and `onmounted` is now dispatched: `create_event_listener`
queues `mounted` listeners into `DioxusState::pending_mounted`, and
`DioxusDocument::take_pending_mounted` drains them (resolved to blitz node ids)
for the embedder to fire. `mounted.rs` provides the `MountedElement`
`RenderedElementBacking` plus a `MountedBackend` trait — the transport that
actually touches the live document, implemented in `dioxus-native` so this crate
stays free of any winit/shell dependency.

Vendoring the crate locally means Loki can build against a known snapshot and
apply targeted fixes without being blocked by an upstream release. See
`docs/editing/input-event-audit.md` — the **Blockers** section — for a
detailed event-by-event analysis of what works and what panics.

**Root cause:** dioxus-native-dom 0.7.4 is a pre-1.0 crate; many
`HtmlEventConverter` methods are unimplemented stubs that panic if called.
Upstream is aware (the `todo:` message in each `unimplemented!()` call names
the missing blitz support), but the fixes depend on blitz-dom adding the
corresponding event infrastructure.

**Upstream status:** No standalone issue filed as of 2026-05-03. The
unimplemented converters are tracked locally in
`docs/editing/input-event-audit.md`. Upstream repository is
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus).

**Removal condition:** Remove when dioxus-native-dom upstream implements the
event converters Loki requires — at minimum `convert_composition_data` (IME)
and `convert_touch_data` (mobile) — and publishes a 0.7.x release that does
not panic for those paths. Before removing, verify with the event availability
table in `docs/editing/input-event-audit.md` that all "Required for editing"
events are available without panicking.

**Added:** 2026-05-02 (introduced in the cursor positioning commit).

---

### blitz-shell — 0.2.3

**Source:** `patches/blitz-shell/` (local, vendored from crates.io version 0.2.3,
checksum `61ecda230035f39b13383f08e0cfc7159c92d194650ac8d57871a207ea0e52b7`).

**Fixes:** `WindowEvent::Touch` events are discarded in the upstream
`handle_winit_event` match arm (the arm body is `{}` with a
`// Todo implement touch scrolling` comment). This patch synthesises touch
contacts as mouse events so `ontouchstart`, `ontouchmove`, and `ontouchend`
handlers fire in loki-text components.

**Implementation approach:** Synthesis as mouse events, not native touch
forwarding. `blitz-traits::events::UiEvent` (0.2.x) has no touch variants —
only `MouseMove`, `MouseUp`, `MouseDown`, `KeyUp`, `KeyDown`, and `Ime`.
Synthesis is therefore the only path available; it reuses all existing
hit-test and cursor infrastructure without requiring changes to blitz-dom.

A `TouchState` struct and `touch_start: Option<TouchState>` field are added
to `View` to track in-progress touch contacts for long-press detection.
Constants `TOUCH_SLOP_PX` (8.0 logical px) and `LONG_PRESS_DURATION` (500 ms)
gate scroll vs. tap vs. long-press classification.

**Soft-keyboard / IME on focus:** Upstream calls `set_ime_allowed(true)` once,
unconditionally, at window creation. On Android that maps to
`AndroidApp::show_soft_input`, which is a no-op before the window is focused —
so the on-screen keyboard never appears, and there is no later trigger. This
patch instead starts with the IME disabled and drives it from DOM focus:
`update_ime_for_focus` runs after every focus-changing event (click / tap /
Tab) and calls `Window::set_ime_allowed(true)` only when the focused node is a
text-editing surface — an `<input>`/`<textarea>`, or any element carrying an
`inputmode` attribute that is not `"none"`. The Loki editor canvas is a
focusable `<div inputmode="text">`, so tapping it raises the keyboard while
tapping a ribbon `<button>` (focusable, but not a text target) lowers it. An
`ime_active: bool` field debounces redundant winit calls.

**Scroll re-sync on resize (PATCH(loki), 2026-06-12):** `resync_scroll_geometry`
re-dispatches `onscroll` (via `collect_scroll_containers` + `handle_scroll_changes`)
to every scroll container with its fresh client geometry. Called from the
`Resized` handler and, through `View::resync_scroll_geometry` (now `pub`), from
dioxus-native's `flush_mounted` when a scroll container mounts. This is what
lets the editor's width-driven reflow / view-mode default react to a window
resize, to the first real Android size, and to the canvas appearing after an
async document load — without the user having to scroll first.

**Root cause:** Upstream has a `// Todo implement touch scrolling` comment at
the touch arm — the feature is planned but not implemented. The IME call is a
hard-coded `// TODO: make this conditional on text input focus`. Upstream also
has no mechanism to notify embedders of element size changes (no
`ResizeObserver` / resize events).

**Upstream status:** No known issue filed as of 2026-05-08. Monitor blitz-shell
releases for native touch implementation.

**Removal condition:** Remove when blitz-shell implements `WindowEvent::Touch`
forwarding natively in a published release and blitz-traits adds `UiEvent`
touch variants.

**Added:** 2026-05-08

---

### blitz-net — 0.2.1

**Source:** `patches/blitz-net/` (local), vendored from the crates.io release of
`blitz-net 0.2.1`. Only `Cargo.toml` is modified; `src/lib.rs` is unchanged.

**Fixes:** The crates.io release of `blitz-net 0.2.1` depends on `reqwest`
with default features, which includes `native-tls`. `native-tls` dynamically
links `libssl.so` at runtime. Android does not ship `libssl.so` as a system
library (it ships `libssl.so.3` in some images, but the path and soname differ
from what OpenSSL expects). The result is:

```
java.lang.UnsatisfiedLinkError: dlopen failed: library "libssl.so" not found
```

in `WryActivity.<clinit>` at `System.loadLibrary("main")` — the app crashes
before any Rust code runs.

The patch switches `reqwest` to `{ default-features = false, features =
["rustls-tls-webpki-roots"] }`. `rustls-tls-webpki-roots` is a pure-Rust TLS
stack that embeds the Mozilla trust bundle; it requires no system OpenSSL.

**Root cause:** `blitz-net` did not gate the TLS backend behind a Cargo
feature, so Android callers have no way to opt out of `native-tls` without
a source patch.

**Upstream status:** No issue filed as of 2026-05-24. Monitor blitz-net
releases for a `rustls` or configurable-TLS feature.

**Removal condition:** Remove when blitz-net upstream ships a version that
uses rustls by default or provides a feature flag to disable native-tls.

**Added:** 2026-05-24

---

### dioxus-native — 0.7.4

**Source:** `patches/dioxus-native/` (local), vendored from the crates.io
release of `dioxus-native 0.7.4`. Only `src/dioxus_application.rs` is
modified; all other files are unchanged.

**Fixes:** `document::Style {}` components send `CreateHeadElement` events via
the winit event-loop proxy during `initial_build()`. These events are processed
in `DioxusNativeApplication::handle_blitz_shell_event()`:

```rust
DioxusNativeEvent::CreateHeadElement { .. } => {
    doc.create_head_element(name, attributes, contents);
    window.poll(); // returns false — no VirtualDom work pending
    // ← no request_redraw() here
}
```

After CSS is applied, `window.poll()` returns `false` (no reactive VirtualDom
work was triggered by a style insertion) so `request_redraw()` is never called.

On desktop (Windows/macOS), this is masked because the OS posts a
`WindowEvent::Resized` event immediately after the window is created, which
calls `with_viewport()` → `request_redraw()` — causing a re-render that picks
up the newly applied CSS. On Android, no such automatic event is posted after
`resumed()`, so the screen remains blank (wgpu clear color is white).

Additionally, the `window.request_redraw()` call in `resumed()` at line 153 of
the original is a no-op: `View::request_redraw()` guards on
`self.renderer.is_active()`, and the renderer is not yet active at that point
(it is activated by the subsequent `self.inner.resumed(event_loop)` call).

The patch adds `window.request_redraw()` after `window.poll()` in the
`CreateHeadElement` handler, ensuring CSS changes always trigger a repaint.

**`MountedData` / programmatic scroll (PATCH(loki), 2026-06-11).** Two
`DioxusNativeEvent` variants are added — `ScrollNode` (absolute scroll, backing
`MountedData::scroll`) and `QueryNodeGeometry` (a one-shot-reply geometry read,
backing `get_scroll_offset` / `get_scroll_size` / `get_client_rect`). A
`ProxyMountedBackend` (impl of dioxus-native-dom's `MountedBackend`) posts these
events through the event-loop proxy. `flush_mounted` drains
`DioxusDocument::take_pending_mounted` after each poll and dispatches the
`mounted` event with a `MountedElement` backing, so `onmounted` fires. This is
what enables the editor's draggable scrollbar thumb.

**Root cause:** Upstream assumed OS-level redraw events would cover the
CSS-application step; this assumption holds on desktop but not on Android.
Upstream also leaves `onmounted` / `MountedData` unimplemented for native.

**Upstream status:** No issue filed as of 2026-05-24. Upstream repository is
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus).

**Removal condition:** Remove when upstream dioxus-native calls
`request_redraw()` after applying head elements, or when the event processing
is made synchronous (the `todo(jon)` comment in the original acknowledges this).

**Added:** 2026-05-24

---

### loki-file-access — 0.1.2

**Source:** `patches/loki-file-access/` (local), vendored from the git source at
commit `176b590fb2da82b2ab278a15b34f0bea56ae0a7a` of
[appthere/loki-file-access](https://github.com/appthere/loki-file-access).

**Fixes:** Two Android-specific bugs that caused a crash when tapping "Open
File" on a NativeActivity (cargo-apk) build:

1. **Wrong activity reference for `startActivityForResult`.** android-activity
   v0.6 intentionally stores the `Application` object (not the `Activity`) in
   `ndk_context`, because `Application` outlives the Activity lifecycle.
   `startActivityForResult` only exists on `Activity`, so calling it on the
   `Application` object threw a `java.lang.NoSuchMethodError` / ART abort. The
   patch adds `init_android(activity_as_ptr)` — called from `android_main` with
   `AndroidApp::activity_as_ptr()` — which stores the actual NativeActivity
   `GlobalRef` in an `AtomicPtr<c_void>`. `start_activity_for_result` now
   prefers this pointer over `ndk_context::android_context().context()`.

2. **JNI exception not cleared on failure.** When `startActivityForResult`
   failed (e.g., called on the wrong receiver type), a JNI exception was left
   pending. The next `FindClass` JNI call made while an exception was pending
   caused ART's checked-JNI mode to abort the process. The patch calls
   `env.exception_clear()` when `call_method` returns an error.

3. **Fail-fast for NativeActivity without Java shim.** `ANativeActivityCallbacks`
   has no `onActivityResult` field — NDK NativeActivity can never receive
   `startActivityForResult` results. Rather than hanging the async task
   indefinitely, the patch returns `Err(PickerError::Platform)` immediately with
   an explanatory message when the NativeActivity pointer is set but no Java
   `FilePickerActivity` shim is registered.

4. **Pre-wired JNI callback for future Gradle build.** The function
   `Java_com_appthere_loki_FilePickerActivity_nativeOnResult` is exported from
   the binary. Once a Gradle-based build includes `FilePickerActivity.kt`
   (calling `nativeOnResult` from `onActivityResult`), end-to-end file picking
   will work without further changes to this crate.

**Also fixes:** `jni::errors::JniError` → `jni::errors::Error` in `jvm_err` and
`attach_err` helpers (the original used the wrong variant type for the jni
0.21.x API), and corrects a `#[no_mangle]` → `#[unsafe(no_mangle)]` attribute
for Rust 2024 edition compatibility.

**Adds (PATCH(loki), 2026-06-13):** `query_window_insets_dp(activity_ptr)` —
orientation-aware safe-area insets `(top, bottom, left, right)` in dp from
`decorView.getRootWindowInsets().getInsets(systemBars | displayCutout)`. Unlike
the existing `query_insets_dp` (which reads the orientation-independent
`status_bar_height` / `navigation_bar_height` resources), this reflects the real
per-side insets, so landscape — where the navigation bar / cutout move to a side
— is padded correctly instead of keeping the portrait top/bottom values. Needs
the **Activity** (passed in via `AndroidApp::activity_as_ptr()`), since
`ndk_context` holds the `Application`, which has no window. Returns `None`
(caller falls back to `query_insets_dp`) before the view is attached or on
API < 30. loki-text re-queries it on resize via a hidden scroll-container
sensor and pushes the result into `appthere_ui::update_safe_area_insets`.

**Root cause:** loki-file-access 0.1.2 was designed for desktop and WASM; the
Android implementation was scaffolded but never exercised on a real NativeActivity
build before this patch.

**Upstream status:** The appthere/loki-file-access repository is maintained by
the same team. These fixes should be pushed upstream and the patch removed once
they are merged and a new version is published.

**Removal condition:** Push these fixes to `appthere/loki-file-access`, publish
a new version, and update the workspace dependency to point at the registry
version. The `[patch."https://github.com/appthere/loki-file-access"]` entry and
the `patches/loki-file-access/` directory can then be removed. Full end-to-end
file picking additionally requires a Gradle build with `FilePickerActivity.kt`.

**Added:** 2026-05-25

---

### blitz-dom — 0.2.4

**Source:** `patches/blitz-dom/` (local).

**Fixes:**

1. **Click-to-focus for non-input elements.** Upstream `handle_click` walks up
   the DOM but calls `clear_focus()` for any element that isn't
   input/label/a — clicking a wgpu canvas cleared keyboard focus from the
   nearest `tabindex="0"` ancestor, preventing `onkeydown` from firing. The
   patch checks `is_focussable()` and calls `set_focus_to()` instead.

2. **Scroll-change collection (PATCH(loki), 2026-06-10).**
   `scroll_node_by_collect` records each node whose scroll offset changed
   during a scroll gesture (including bubbling), and the `Document` trait
   gains a default-no-op `handle_scroll_changes` hook. blitz-shell calls the
   hook after wheel/touch scrolling; dioxus-native-dom implements it on
   `DioxusDocument` to dispatch DOM `scroll` events (with `NativeScrollData`
   payloads) into the VirtualDom, so Dioxus `onscroll` handlers fire.
   Routed through the `Document` trait because blitz-traits 0.2 has no
   scroll `DomEventData` variant.

3. **Absolute scroll (PATCH(loki), 2026-06-11).** `scroll_node_to_collect`
   scrolls a node to an absolute `(x, y)` offset (clamped, change-collecting),
   implemented on top of `scroll_node_by_collect`. Backs `MountedData::scroll`
   in the dioxus-native patch (draggable scrollbar thumb, scroll-to-cursor).

4. **Scroll-container enumeration (PATCH(loki), 2026-06-12).**
   `collect_scroll_containers` returns every node whose computed overflow is
   `scroll`/`auto`. blitz-shell calls it after a viewport resize (and after a
   scroll container mounts) and feeds the result to `handle_scroll_changes`, so
   the embedder re-receives `onscroll` with the new client size — letting the
   reflow view relayout to the window width without a user scroll.

**Removal condition:** Upstream blitz-dom implements tabindex focus-on-click
for non-input elements, dispatches scroll events to embedders, and exposes an
absolute node-scroll API.

**Added:** 2026-05-18 (focus); extended 2026-06-10 (scroll events) and
2026-06-11 (absolute scroll), together with matching changes in the blitz-shell
and dioxus-native(-dom) patches.

---

### anyrender_vello — 0.6.2

**Source:** `patches/anyrender_vello/` (local), vendored from crates.io 0.6.2.

**Fixes:** Two Mali GPU driver crashes on Android (Pixel 9 / Mali-G715,
driver r54p2) that killed the Vulkan device at startup with
`Device::poll: Validation Error — Parent device is lost`:

1. **Concurrent shader-module creation.** `DEFAULT_THREADS` was `None` on
   Android (Vello then uses one thread per core), and Mali drivers race
   during parallel pipeline compilation. Forced to 1 on Android, matching
   what upstream already does for macOS and what Vello's own `with_winit`
   example does for Android.

2. **Compute-dispatch device loss.** Even with single-threaded init, the
   Mali r54 driver loses the device executing Vello's GPU compute stages on
   the first frame. On Android the renderer is now created with
   `use_cpu: true` (compute stages run on the CPU; fine rasterization and
   the surface presentation stay on the GPU) and area-only antialiasing
   (`AaSupport::area_only()` / `AaConfig::Area`). The same settings are
   applied to the workspace's own Vello renderers in
   `loki-renderer/src/page_paint_source.rs` and `doc_page_source.rs`
   (COMPAT(android-mali) comments).

**Root cause:** Arm Mali driver bugs with Vulkan compute — the same driver
family produces device-lost crashes in other engines (e.g. Godot) on
Pixel 8/9-class devices.

**Upstream status:** Not filed as of 2026-06-10. The `num_init_threads`
Android default is a candidate upstream fix for anyrender_vello; the
`use_cpu` fallback is a Loki-specific mitigation pending a Mali driver fix.

**Removal condition:** A Mali driver update (or wgpu/Vello workaround) that
survives Vello's GPU compute pipeline on Mali-G715, plus an anyrender_vello
release with the Android `num_init_threads` default. Re-test with
`use_cpu: false` and MSAA16 before removing.

**Added:** 2026-06-10

---

## Removing a patch

Before removing a patch:

1. Confirm the upstream release that fixes the issue is in `Cargo.lock`.
2. Remove the `[patch]` entry from `Cargo.toml`.
3. Run `cargo check --workspace` and `cargo test --workspace`.
4. Remove the patch source directory (`patches/<crate>/`).
5. Update or remove the corresponding entry in this file.
