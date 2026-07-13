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

### loki-file-access — 0.1.3 (git `main` @ `d2b7bc5`)

**Source:** `patches/loki-file-access/` (local), vendored 2026-07-13 from
[appthere/loki-file-access](https://github.com/appthere/loki-file-access)
commit `d2b7bc5`. Wired via
`[patch."https://github.com/appthere/loki-file-access"]` in the root
`Cargo.toml`.

**Android Save As fix (PATCH(loki)).** `pick_save` treated a
`takePersistableUriPermission` failure as fatal (`?`), **after**
`ACTION_CREATE_DOCUMENT` had already created the target document — providers
that grant no persistable permission on create results throw
`SecurityException`, so Save As stranded a freshly-created **blank** file and
surfaced an error while the write itself would have succeeded (and plain Save,
which never re-creates, kept working). The patch:

- makes the persistable grant **best-effort** (`tracing::warn!` + continue —
  the session grant from the create result is sufficient for the write;
  persistence only affects reopening after an app restart);
- clears the pending JNI exception on that failure so subsequent JNI calls on
  the thread keep working;
- queries the created document's **real display name** (the user may rename in
  the create dialog) instead of trusting `suggested_name` — the name drives
  format detection on export.

**Removal condition:** upstream `loki-file-access` ships the equivalent fix;
then drop the `[patch]` entry and `patches/loki-file-access/`.

### dioxus-native-dom — 0.7.9

**Version pin:** the whole dioxus family is pinned to `=0.7.9` in the root
`Cargo.toml` (and every crate that declares `dioxus`). This patch is
version-specific; a loose `"0.7"` requirement lets Cargo prefer a newer 0.7.x
from crates.io and **silently drop this patch** — see "Upgrading Dioxus" below.

**Source:** `patches/dioxus-native-dom/` (local), originally vendored from
upstream commit `1eb00b5e0080ab4bd6a11ddd0a01c97f28493e04` in
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus)
(`packages/native-dom/` path). The vendor copy carries local modifications
(`dirty: true` in `.cargo_vcs_info.json`). **Re-vendored 0.7.4 → 0.7.9 on
2026-06-19:** upstream `src/` was byte-identical between the two versions, so
re-vendoring was a manifest version bump only (the loki source modifications
already applied).

**Scroll-event dispatch (PATCH(loki)).** `DioxusDocument::handle_scroll_changes`
dispatches the DOM `scroll` event into the Dioxus `VirtualDom` for each node
whose scroll offset changed (blitz-traits 0.2 has no scroll `DomEventData`
variant), so `onscroll` handlers fire — this is what drives the editor's custom
scrollbar thumb. `mounted.rs` additionally implements `MountedData::scroll`, the
programmatic scroll the scrollbar thumb-drag uses. If this patch is dropped, the
content still scrolls (blitz-shell handles the wheel) but the thumb freezes and
drag is a no-op.

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

**Soft-keyboard re-trigger on tap (PATCH(loki), 2026-06-25):** the OS never
reports when the user dismisses the soft keyboard (Android back / swipe-down),
so `ime_active` stays `true` and a second tap to reposition the caret in the
already-focused canvas would never bring it back. `update_ime_for_focus` now
takes a `force_show` flag: a focus change still toggles IME on/off, but a fresh
pointer release (mouse-up / touch tap) on a text surface re-issues
`set_ime_allowed(true)` even when IME is already active, re-summoning a
dismissed keyboard. (If a future winit dedupes same-value `set_ime_allowed`
calls, switch the force path to a `false`→`true` toggle.)

**Soft-keyboard safe-area re-sync (PATCH(loki), 2026-06-26):** the app runs in a
stock `NativeActivity` with no `windowSoftInputMode`, so the GL surface is **not**
resized when the soft keyboard appears — winit never fires `Resized` and the
keyboard overlays the bottom of the app (ribbon / bottom-of-document content).
winit / Blitz / Dioxus surface no IME-visibility or height events, but we already
drive the keyboard ourselves via `set_ime_allowed`, so a visibility change is our
cue to re-reserve the bottom safe area. When `update_ime_for_focus` shows, hides
or force-re-shows the keyboard, it calls `arm_ime_settle`, which opens a bounded
settle window (`IME_INSET_SETTLE`, 400 ms) and wakes the idle event loop at
60/160/280/400 ms via `BlitzShellEvent::Poll`. While `ime_settle_until` is in the
future, `poll` calls `resync_scroll_geometry`, which re-dispatches `onscroll`; the
app's hidden `SafeAreaResizeSensor` catches that tick and re-queries
`query_window_insets_dp` — whose mask now includes `WindowInsets.Type.ime()`, so
the returned `bottom` grows to the keyboard height. The settle window exists
because Android reports/animates the IME inset a frame or two after the
visibility request (it is the real animation duration, not an arbitrary sleep).
Android-only; on desktop `ime_settle_until` stays `None`. (The former limitation
— a system-back / swipe-down dismissal not re-syncing the bottom padding — is now
closed by the user-driven IME visibility signal below.)

**User-driven soft-keyboard collapse signal (PATCH(loki), 2026-07-01):** the
re-sync above only fires when *the app* drives the keyboard (focus change / caret
tap). When the **user** collapses (or re-summons) the keyboard — system back
button, swipe-down gesture, or the keyboard's own hide key — a `NativeActivity`
gets no winit event and its surface is not resized, so `ime_active` went stale and
the bottom safe area stayed reserved for a keyboard that is gone. This is now
closed with a real Android inset callback: `loki-file-access`'s
`ImeInsetsListener` (a Java shim) is installed on the decor view via
`install_ime_listener` and fires `View.OnApplyWindowInsetsListener` on **every**
IME transition, reading `WindowInsets.Type.ime()` visibility (API 30+; a no-op
below, matching the query fallback). Its native callback — bound with
`RegisterNatives`, so it is independent of the host `.so` name — is bridged in
`android_main` to `blitz_shell::notify_ime_visibility_changed`, which (in
`ime_android.rs`) records the new visibility and wakes the event loop with a
`Poll`. `WindowState::poll` drains it: it mirrors `ime_active` and calls
`arm_ime_settle`, reusing the exact re-sync path above so the bottom safe area
converges to the settled keyboard height (0 on a collapse). The listener passes
insets through (`v.onApplyWindowInsets`), so it does not consume or alter the
system's inset handling. Android-only.

**Scroll re-sync on resize (PATCH(loki), 2026-06-12):** `resync_scroll_geometry`
calls `doc.resolve()` and then re-dispatches `onscroll` (via
`collect_scroll_containers` + `handle_scroll_changes`) to every scroll container
with its fresh client geometry. Called from the `Resized` handler and, through
`View::resync_scroll_geometry` (now `pub`), from dioxus-native's `flush_mounted`
whenever an element with an `onmounted` listener mounts. This is what lets the
editor's width-driven reflow / view-mode default react to a window resize, to the
first real Android size, and to the canvas appearing after an async document
load — without the user having to scroll first.

**Important:** `flush_mounted` only resyncs when an `onmounted` listener is
*pending*, i.e. when a node carrying `onmounted` has just mounted. The editor's
scroll container mounts once (with a one-page loading placeholder), so when the
real multi-page document later mounts *inside* it, the container does not
re-mount and its Taffy scroll overflow would stay stale — leaving the wheel
unable to scroll (the container looks non-scrollable until a mouse-move forces a
re-resolve) and the scrollbar thumb sized for one page. `loki-renderer`'s
`DocumentView` therefore attaches an (empty) `onmounted` to its content root so
this resync fires the moment the document content mounts. If you change the
resync trigger, keep that contract in mind.

**Wheel/touch scroll target the document, never the UI (PATCH(loki),
2026-06-20):** the `MouseWheel` handler scrolls the hovered node first, then
falls back to the *focused* node, and **never** the root viewport. Two parts:

- *First-paint scroll.* The hover node is updated only on cursor-move events, so
  immediately after navigating to a new view (e.g. opening a document) it is
  either unset *or stale* — left pointing at a node from the previous view that
  scrolls nothing. The original form (`hover.or_else(focused)`) only consulted
  the focused node when hover was `None`, so a stale-but-present hover node
  swallowed the gesture and the wheel did nothing until the user moved the mouse.
  The handler now treats a hover node that consumed no scroll as "no target" and
  falls through to the focused node. The editor canvas is a focusable scroll
  container that is focused on mount (see the `autofocus` patch below), so the
  wheel scrolls it immediately on first paint.

- *No root-viewport scroll.* Both the wheel and touch-drag handlers now use
  `scroll_node_within_collect` (blitz-dom), which is identical to
  `scroll_node_by_collect` except that scrolling which bubbles past the root
  element is dropped rather than nudging the viewport. The Loki shell is a fixed
  full-window layout with no scrollable root, so a gesture that runs off the end
  of the document — or starts over a non-scrolling element like the ribbon —
  must do nothing instead of shifting the whole UI by the sub-pixel slack
  between the root content and the window (a long-standing ~1px "UI jiggle").

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

**Hover tooltip overlay (PATCH(loki), 2026-06-25):** Blitz/Stylo do not support
`position: absolute`/`fixed`, so a hover tooltip cannot be a DOM element (see the
COMPAT note in `appthere-ui/.../ribbon/button.rs`). Instead the shell paints the
tooltip **into the Vello scene itself**, after `paint_scene`, entirely outside
the DOM (`src/tooltip.rs` + `View::render_scene`). On `CursorMoved` the shell
hit-tests the node under the cursor (`doc.hit`) and walks ancestors for a `title`
attribute; a new titled element arms a delayed show (`HOVER_DELAY` = 500 ms) by
spawning a one-shot thread that sends `BlitzShellEvent::Poll` at the deadline
(the loop is `ControlFlow::Wait`, so a stationary cursor produces no other
wake). `poll` flips the tooltip visible and requests a redraw; `render_scene`
then shapes the label with a self-contained parley `FontContext` (`()` brush,
generic sans-serif) and draws a shadow + rounded-rect + glyphs via
`PaintScene::{draw_box_shadow, fill, draw_glyphs}`, mirroring `blitz-paint`'s
glyph bridge so `run.font()` matches the `peniko::FontData` `draw_glyphs`
expects (hence the pinned `parley 0.6` / `peniko 0.5` / `kurbo 0.12` deps).
Click / scroll / keypress / touch clear it. The app side just adds a `title`
attribute (the ribbon icon button reuses its `aria_label`). **Removal
condition:** remove when Blitz supports `position: absolute`/`fixed` so tooltips
can be real DOM nodes.

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

### dioxus-native — 0.7.9

**Version pin:** pinned to `=0.7.9` (see the dioxus-native-dom entry above and
"Upgrading Dioxus" below).

**Source:** `patches/dioxus-native/` (local), originally vendored from the
crates.io release of `dioxus-native 0.7.4`. `src/dioxus_application.rs`,
`src/dioxus_renderer.rs`, and `src/lib.rs` carry loki modifications; the
manifest also carries loki customisations (Android Mali `softbuffer` workaround,
the `android_gpu` cfg lint, extra deps). **Re-vendored 0.7.4 → 0.7.9 on
2026-06-19:** upstream `src/` was byte-identical between the two versions, so
only the dioxus-family version requirements in the hand-maintained `Cargo.toml`
were bumped (the loki manifest customisations preserved).

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

**`autofocus` enabled by default (PATCH(loki), 2026-06-20).** `autofocus` is
added to the `default` feature set in `patches/dioxus-native/Cargo.toml` (it
forwards to `blitz-dom/autofocus`). Upstream ships this feature **off**, and the
`dioxus` meta crate's `native` feature does not turn it on, so an element with
`autofocus="true"` was never focused on mount. The Loki editor canvas declares
`autofocus="true"` so the user can type — and scroll with the wheel — the moment
a document opens, without clicking first; this re-enables that intended
behaviour. When re-vendoring the manifest during a Dioxus upgrade, preserve this
addition (it is a loki customisation, like the Android `softbuffer` deps).

**Bundled-font pre-registration (PATCH(loki), 2026-06-27).** `Config` gains a
`font_blobs: Vec<Vec<u8>>` field and a `with_fonts(..)` builder; `launch_cfg_with_props`
moves those blobs into `DocumentConfig.extra_fonts` (see the blitz-dom entry) so
the renderer registers them into its parley `FontContext` synchronously at
startup. The apps pass `loki_fonts::ui_font_blobs()` (the Atkinson Hyperlegible
Next UI typeface plus the metric-compatible fallback families). This replaces the
previous approach of injecting an `@font-face` `data:` URI via `document::Style`,
which relied on the asynchronous network-provider resource fetch and did not load
the UI typeface on Android (the chrome fell back to a wide system font). The
bytes are known at compile time, so synchronous registration is the correct
layer. Preserve this `Config`/launch customisation when re-vendoring.

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

5. **Non-viewport-bubbling scroll (PATCH(loki), 2026-06-20).**
   `scroll_node_within_collect` mirrors `scroll_node_by_collect` but drops any
   scroll that bubbles past the root element instead of moving the viewport
   (both delegate to a shared `scroll_node_by_collect_inner` taking a
   `bubble_to_viewport` flag). blitz-shell's wheel and touch handlers use it so
   the fixed full-window Loki shell never scrolls as a whole — a gesture that
   overruns the document, or starts over the ribbon, does nothing rather than
   jiggling the UI by the sub-pixel root/window slack.

6. **Static canvases don't force a per-frame redraw (PATCH(loki), 2026-06-21).**
   `is_animating()` returns `has_canvas | has_active_animations`, and the shell's
   redraw loop re-requests a redraw every frame while it is true. Loki paints
   every document page as a `<canvas src>` custom-paint tile, so `has_canvas` is
   permanently true — the app spun in a **continuous idle render loop**: high
   CPU/battery, and per-frame GPU resource churn that grew RSS without bound even
   with the app untouched (observed climbing past 3 GB at idle). A new
   `BaseDocument::needs_animation_tick()` returns only `has_active_animations`
   (genuine CSS animations/transitions), and blitz-shell's `redraw()` re-arms on
   that instead of `is_animating()`. Loki's canvas tiles are static between
   events — their content only changes via DOM mutations (the tile's
   `data-cursor`/generation attribute, scroll remounts, viewport resize), each of
   which already schedules a redraw — so dropping the canvas-forced loop leaves
   updates correct while idle frames stop. (`is_animating()` is left intact for
   any other consumer.)

7. **Embedder-supplied font blobs (PATCH(loki), 2026-06-27).** `DocumentConfig`
   gains `extra_fonts: Vec<Vec<u8>>`; `BaseDocument::new` registers each blob into
   the parley `FontContext` (on top of the system fonts and the default bullet
   font) at construction. This lets an app bundle its UI/fallback fonts and have
   them resolve **synchronously** on every platform, instead of relying on the
   asynchronous `@font-face` `data:` URI resource-fetch path (which did not load
   the UI typeface on Android). `dioxus-native`'s `Config::with_fonts(..)` feeds
   this field; the Loki apps pass `loki_fonts::ui_font_blobs()`.

**Removal condition:** Upstream blitz-dom implements tabindex focus-on-click
for non-input elements, dispatches scroll events to embedders, exposes an
absolute node-scroll API, and stops treating a static canvas as perpetually
animating (e.g. a per-source "needs animation" signal).

**Added:** 2026-05-18 (focus); extended 2026-06-10 (scroll events),
2026-06-11 (absolute scroll), 2026-06-21 (`needs_animation_tick` — stop the
idle canvas redraw loop, paired with the blitz-shell `redraw()` change), and
2026-06-27 (`extra_fonts` — synchronous bundled-font registration), together
with matching changes in the blitz-shell and dioxus-native(-dom) patches.

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

**Additional fix (texture release on teardown):** `CustomPaintSource` gained a
`fn release(&mut self, ctx: CustomPaintCtx)` method (default no-op), and
`VelloWindowRenderer::unregister_custom_paint_source` now calls it (while the
renderer is `Active`) before suspending and dropping the source.

- *Root cause:* a texture handed to the renderer via
  `CustomPaintCtx::register_texture` lives in the renderer's texture registry
  until `unregister_texture` is called. The only teardown hook a source had was
  `suspend()`, which takes no `CustomPaintCtx` and so cannot unregister. When a
  paint source is unregistered (e.g. a virtualized page tile scrolling out of
  view), its last-registered full-resolution texture (~10+ MB) leaked in the
  registry. Scrolling a long document top→bottom→top grew RSS unboundedly
  (observed ~500 MB → ~1.3 GB) and never recovered. App-level `suspend()` did
  not leak because the whole renderer is recreated on resume; only per-source
  unregister was affected.
- *Loki consumer:* `loki-renderer/src/page_paint_source.rs` (`LokiPageSource`)
  implements `release` to `unregister_texture` its page texture.
- *Upstream status:* candidate upstream fix — the custom-paint API has no other
  way to release per-source textures on teardown.
- *Removal condition:* an anyrender_vello release whose custom-paint teardown
  releases a source's registered textures (e.g. an equivalent `release`/`drop`
  hook), at which point `LokiPageSource::release` can target the upstream API.

**Updated:** 2026-06-21

---

## Upgrading Dioxus

Dioxus is pinned to an exact version (`=X.Y.Z`) in **every** crate that declares
it, because two patches (`dioxus-native`, `dioxus-native-dom`) are vendored at
that version. **Never just bump the version number** — Cargo will prefer the
crates.io release over a stale-versioned patch and silently drop it
(`warning: Patch ... was not used`), breaking scrolling, drag, `onmounted`,
touch, and IME with no compile error. Re-vendor the two patches first.

Let `OLD` be the current pin and `NEW` the target (e.g. `OLD=0.7.4`,
`NEW=0.7.9`).

1. **Fetch pristine upstream sources** for both versions of both patched crates,
   so you can see exactly what upstream changed and what loki changed:

   ```bash
   tmp=$(mktemp -d)
   for c in dioxus-native dioxus-native-dom; do
     for v in "$OLD" "$NEW"; do
       curl -fsSL "https://static.crates.io/crates/$c/$c-$v.crate" \
         | tar xz -C "$tmp"        # extracts $tmp/$c-$v/
     done
   done
   ```

2. **Check how much upstream changed** between `OLD` and `NEW`:

   ```bash
   diff -rq "$tmp/dioxus-native-dom-$OLD/src" "$tmp/dioxus-native-dom-$NEW/src"
   diff -rq "$tmp/dioxus-native-$OLD/src"     "$tmp/dioxus-native-$NEW/src"
   ```

   - **No source differences** (as for 0.7.4 → 0.7.9): the existing patched
     `src/` already matches `NEW`; the re-vendor is a **manifest bump only**
     (steps 4–5).
   - **Source differences**: do a **3-way merge** per changed file — the loki
     delta is `diff(pristine-OLD, patches/<crate>)`; re-apply it onto the
     `pristine-NEW` file (the loki edits are marked `PATCH(loki)`). Replace the
     patch `src/` with `pristine-NEW` + the re-applied loki edits, then continue.

3. **Confirm what loki customised in each manifest** (so you preserve it):

   ```bash
   diff "$tmp/dioxus-native-dom-$OLD/Cargo.toml" patches/dioxus-native-dom/Cargo.toml
   diff "$tmp/dioxus-native-$OLD/Cargo.toml"     patches/dioxus-native/Cargo.toml
   ```

4. **Update each patch manifest to `NEW`:**
   - If loki did **not** customise it (e.g. `dioxus-native-dom`): copy the
     pristine `NEW` manifest verbatim —
     `cp "$tmp/dioxus-native-dom-$NEW/Cargo.toml" patches/dioxus-native-dom/Cargo.toml`.
   - If loki **did** customise it (e.g. `dioxus-native`): bump the crate
     `version` and the `dioxus-*` dependency requirements `OLD → NEW` **in
     place**, preserving the loki customisations.
   - Either way the patch crate's own `version` must equal `NEW` so it matches
     what `dioxus = "=NEW"` pulls in.

5. **Move the pin** in every crate that declares dioxus:

   ```bash
   for f in Cargo.toml loki-renderer/Cargo.toml appthere-canvas/Cargo.toml \
            loki-text/Cargo.toml loki-presentation/Cargo.toml loki-spreadsheet/Cargo.toml; do
     sed -i "s/version = \"=$OLD\"/version = \"=$NEW\"/" "$f"
   done
   ```

   Also update the pin comment in the root `Cargo.toml` and the version in the
   two patch section headers in this file.

6. **Re-resolve the lockfile** for the whole dioxus family:

   ```bash
   PKGS=$(grep -oE 'name = "dioxus[a-z-]*"' Cargo.lock | sed 's/name = //;s/"//g' | sort -u | tr '\n' ' ')
   cargo update $PKGS --precise "$NEW"
   ```

7. **Verify the patches actually apply** (this is the whole point):

   ```bash
   cargo check --workspace 2>&1 | grep -i "was not used"   # must print NOTHING
   grep -A2 'name = "dioxus-native-dom"' Cargo.lock          # version = NEW, no `source` line (= local path)
   ```

   `cargo check --workspace`, `cargo fmt --all`, and
   `cargo clippy --workspace -- -D warnings` must all pass. Finally, run the app
   and confirm scroll-wheel moves the thumb and thumb-drag scrolls the page.

8. **Update docs:** the two patch section headers and re-vendor dates here, and
   the Dioxus pin note in `CLAUDE.md`.

## Removing a patch

Before removing a patch:

1. Confirm the upstream release that fixes the issue is in `Cargo.lock`.
2. Remove the `[patch]` entry from `Cargo.toml`.
3. Run `cargo check --workspace` and `cargo test --workspace`.
4. Remove the patch source directory (`patches/<crate>/`).
5. Update or remove the corresponding entry in this file.

## Removed patches

### loki-file-access — removed 2026-07-05 (was 0.1.2)

The `patches/loki-file-access` patch (added 2026-05-25) carried the Android
NativeActivity fixes — `init_android` Activity `GlobalRef` for
`startActivityForResult`, JNI exception clearing, the fail-fast for missing
`FilePickerActivity`, the `FilePickerActivity`/`ImeInsetsListener` Java shims
+ dexing `build.rs`, `query_window_insets_dp` / `install_ime_listener`, and
`FileAccessToken::delete()` / `copy_bytes_to()` — plus jni 0.21 error-type and
Rust-2024 `#[unsafe(no_mangle)]` fixes.

Removed when the full patch content was upstreamed to
[appthere/loki-file-access](https://github.com/appthere/loki-file-access) as
**0.1.3** (commit `d2b7bc5`, fast-forwarded to `main`; the workspace's
`branch = "main"` git dependency now resolves to it directly). The crate is
not yet published to a registry — if it ever is, the git dependency can be
swapped for the registry version, but nothing requires that. Full end-to-end
Android file picking still requires a Gradle build that bundles the (now
upstream) `FilePickerActivity` shim.

### fontique — removed 2026-06-21 (was 0.8.0)

The `patches/fontique` patch (added 2026-04-13) worked around two issues with
the crates.io publication of **fontique 0.8.0**: (1) a missing
`fontconfig_sys = { package = "yeslogic-fontconfig-sys", … }` alias dropped
during the publish pipeline, and (2) a dlopen/static feature-unification
conflict with blitz-dom's fontique 0.6.

Removed when Loki's own crates moved from fontique 0.8 to **fontique 0.10**
(alongside the parley 0.8 → 0.10 upgrade). fontique 0.10 restores the
`fontconfig_sys` alias, so issue (1) no longer applies. Issue (2) is now
handled without a patch by enabling the `fontconfig-dlopen` feature directly on
`loki-layout`'s fontique dependency (fontique is re-exported through parley, so
this turns dlopen on wherever fontique appears — including crates such as
`loki-vello` whose graph does not contain blitz-dom). blitz-dom's own fontique
0.6 continues to enable `yeslogic-fontconfig-sys/dlopen`, so both fontique
generations agree on linkage mode.
