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

**`FileAccessToken::from_path` (PATCH(loki), 2026-08-11, §13).** The token
type deliberately has no public constructor — pickers mint tokens — but OS
launch surfaces (argv "Open with", file-manager double-click, and later the
single-instance forward) hand the app a bare path with no picker in the
loop. `from_path` (desktop targets only) canonicalizes the path — which also
verifies existence, so a mistyped argument fails at construction with the
OS error — and builds the `Desktop` token variant. Used by
`loki-text::routes::startup_open`.

**Removal condition:** upstream `loki-file-access` ships the equivalent fix
**and** a public `from_path`; then drop the `[patch]` entry and
`patches/loki-file-access/`.

### appthere-color — 0.1.1 (vendored, not patched)

**Source:** `patches/appthere-color/`, copied unmodified from the crates.io
`0.1.1` tarball. Full detail — the compliance measurement, what was deliberately
*not* changed, and the re-vendoring procedure — is in
[`patches/appthere-color/VENDORED.md`](../patches/appthere-color/VENDORED.md).

**This entry is different in kind from every other one on this page.** The rest
exist because upstream has a defect, and each names the upstream fix that would
let it be deleted. This one has no defect: `appthere-color` is AppThere's own
crate, and Spec 08 Phase 5's colour work changes it and its consumers together,
so vendoring removes a crates.io release cycle from each iteration. It is a
working copy, not a workaround.

**Local modifications:** none as of the fork point.

**Removal condition:** when Phase 5's colour work is finished and the API has
settled, publish the accumulated changes as `0.1.2+` and delete the entry. An
entry still present with no local modifications has outlived its reason — which
is the check to make, because a vendoring that costs nothing to keep is also one
nobody notices keeping.

**Added:** 2026-08-02 (Spec 08 T5.1 / D-09).

---

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

**Wheel-event dispatch (PATCH(loki), 2026-08-02).** `DioxusDocument::handle_wheel`
dispatches a bubbling DOM `wheel` event, and `convert_wheel_data` is implemented
against a new `NativeWheelData` (exported from the crate root). Before this,
`onwheel` was `unimplemented!()` and no wheel gesture reached Dioxus at all —
blitz-shell consumed the whole event. Three things in it are deliberate:

- **It bubbles, and starts at the nearest Dioxus-mapped ancestor.** A wheel's
  target is a hit-test result, routinely a text node with no `data-dioxus-id`;
  dispatching only at mapped nodes (as the scroll path does) would make the hook
  silent in the case it exists for. The nodes stepped over carry no Dioxus
  listeners, so nothing is skipped.
- **`NativeWheelData` carries a scrollport-relative position** as well as the
  usual target-relative `element_*`. The two frames differ by however far the
  target sits inside the scrolled content, and a consumer cannot convert between
  them — recovering the scrollport frame needs the target's position within it,
  which is exactly what the event does not carry. Pointer-anchored zoom needs the
  scrollport frame; a sitting measured the gap at 135 px. It comes from
  `BaseDocument::scrollport_origin`.
- **`trigger_button()` is `None` and `held_buttons()` empty.** A wheel turn is
  not a button press, and the shell tracks no button state on this path.

If this patch is dropped, Ctrl+wheel zoom stops working (ordinary scrolling is
unaffected — that is blitz-shell's).

**Element-origin fix (PATCH(loki), 2026-08-02).** `element_*` coordinates now
come from `Node::border_box_position` rather than `absolute_position`, which
subtracts the node's *own* scroll offset and so places the origin at the top of
the node's scrolled content. Invisible on the click path (a click targets a leaf,
and a leaf scrolls nothing) and wrong by the full scroll offset the moment the
target is a scroll container.

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

**Programmatic focus (PATCH(loki), 2026-08-02).**
`RenderedElementBacking::set_focus` is a trait method with a `NotSupported`
default, and the vendored backing did not override it — so **no component in
this workspace could move focus at all**. That is disqualifying for any overlay:
returning focus to the control that opened a menu is the single most-relied-on
behaviour of the class, and losing it to the document root on close is the
commonest accessibility defect there is. It surfaced in Spec 08 T4.5, where
`focus_after_dismiss` and `dismiss_sequence` turned out to be decisions with no
*possible* caller rather than ones with a forgotten caller.

`MountedElement::set_focus` now routes to a new `MountedBackend::focus_node`,
implemented in `dioxus-native` as a `DioxusNativeEvent::FocusNode` posted to the
event loop — exactly the shape `scroll` already used for `scroll_node_to`. On
the event-loop side it calls `BaseDocument::set_focus_to` / `clear_focus`, both
of which already existed and are already driven by the mouse path; the handler
checks `get_node` first, because focus restoration on dismissal races the
unmount that caused it and a stale node id must not reach `set_focus_to`.

Fire-and-forget, like `scroll`: the document lives on the event-loop side, so a
round trip would make focus restoration await a frame it is racing. Callers
therefore get `Ok(())` meaning *posted*, not *focused*.

If this patch is dropped, every popover stops returning focus on dismissal and
`appthere_ui::components::popover::focus_tests::restoring_focus_to_the_anchor_is_a_real_action`
fails — deliberately, so the loss is a test failure rather than a silent
regression in behaviour nothing asserts.

**Focus preservation must not overrule a deliberate move (PATCH(loki),
2026-08-02).** `DioxusDocument::poll` captures the focused node's Dioxus
`ElementId` before `render_immediate` and restores it after, so focus survives a
re-render that rebuilds the focused node. The mutation flush is also where
`autofocus` fires — so a render that *deliberately* moved focus (mounting an
`autofocus` overlay) had it handed straight back to whatever held it before.
Combined with the click-focus ordering above, an overlay could be focused twice
and lose it twice, and the symptom was identical either way: a menu opens and
every key still goes to the trigger.

The restoration is now skipped when focus changed during the render *to a node
that still exists*. Focus that is merely stale — its node removed, or its id
reused — is the case this restoration exists for, and it is exactly the case
where the current focus does not name a live node, so the discriminator is a
before/after pair rather than a smarter element lookup.

**Not covered:** "focus the node *after* this one", which
`DismissStep::AdvanceFocusPastAnchor` (Tab out of a menu) needs.
`set_focus` takes a `bool`, and blitz-dom's `focus_next_node` — which would
serve it — is reachable only by depending on the renderer crate from
`appthere-ui`. Tracked in `scripts/pending-questions.txt`.

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

**Key events with no scancode are no longer dropped (PATCH(loki), 2026-08-16).**
The `KeyboardInput` arm opened with `let PhysicalKey::Code(key_code) =
event.physical_key else { return; }`, discarding the **entire** event. But the
scancode is used only by the Ctrl/Alt shortcut tables a few lines below; the
text dispatch at the end of the arm reads `logical_key`/`text` and never touches
it. So the guard silently swallowed text-bearing events.

That is not hypothetical on Android: a `NativeActivity` gives the IME no
`InputConnection`, so the soft keyboard *synthesises* key events, and any
keycode outside winit's Android translation table arrives as
`PhysicalKey::Unidentified`. The symptom is occasional missing characters —
input that vanishes with no error — rather than a reproducible failure, which is
why it survived: nothing fails, some keystrokes just never happen.

`key_code` is now an `Option` that gates only the shortcut tables. Everything
else reaches the DOM.

**Removal condition:** upstream `blitz-shell` restricting the early return to
the shortcut handling (or moving the shortcut tables to `logical_key`).

**Not fixed here, and not fixable at this layer:** winit's Android backend maps
`KeyAction::Multiple` (the deprecated `KEYCODE_UNKNOWN` + string payload path,
which some IMEs still use) to `ElementState::Released`, so such an event now
reaches the DOM as a *key-up* and still inserts nothing. Fixing that needs a
winit patch, which this workspace does not carry.

**Wheel reporting, and the modified-wheel policy (PATCH(loki), 2026-08-02).**
The `MouseWheel` arm now reports every gesture to the embedder via
`Document::handle_wheel` *before* deciding whether to scroll with it, and
declines to scroll any wheel carrying a modifier.

Two details are load-bearing. First, the report carries the **platform's own**
delta and unit (`WheelUnit::Lines` / `Pixels`), not the `scroll_x`/`scroll_y`
this handler computes — those have been through a line-to-pixel factor that is a
scroll-speed tuning constant, not a measured line height, so forwarding them
would answer "how far did the wheel turn" with a number meaning "how far should
this scroll". Second, the decline test is **any** modifier rather than Control:
which modifiers mean zoom is the embedder's choice (Ctrl on Windows/Linux, Cmd
→ `SUPER` on macOS), and two modifier lists that have to agree across a crate
boundary is how a wheel comes to zoom *and* scroll at once. The broader set
cannot be narrower than whatever the embedder picks; the cost is that Shift and
Alt no longer scroll, which is no loss — neither had a meaning here.

Only the shell can honour this policy: by the time an embedder sees a
notification, the scroll has already happened.

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

**`FocusNode` (PATCH(loki), 2026-08-02).** A third `DioxusNativeEvent` variant,
backing `MountedData::set_focus` — see the dioxus-native-dom entry for why
programmatic focus did not exist at all before this. `ProxyMountedBackend::focus_node`
posts it; the handler calls `BaseDocument::set_focus_to` (or `clear_focus`) after
checking `get_node`, then polls and requests a redraw so the focus ring and any
`onfocus` handler land in the same frame. Same shape as `ScrollNode`, deliberately:
one transport, one place a node id is validated.

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

### parley — 0.6.0

**Source:** `patches/parley/` (local, vendored from crates.io 0.6.0).

**Fix (one hunk, `src/shape/mod.rs`):** when the shaper breaks a run it refreshes
every field of the current shape item **except** `letter_spacing` and
`word_spacing`. The item therefore keeps the *first* style's spacing for the
whole layout, so a styled run that asks for letter-spacing gets it only when it
is the paragraph's first run — and then every other run gets it too.

**How it was found, and why it took a patch.** ADR-0017 §5.6: the DOM reflow
view's line breaks matched the canvas path on every fixture except a paragraph
with a letter-spaced run in it. Measured with `advance_probe`'s structural rows,
one span against three: a tracked span **first** widened all 36 characters of the
row (+288 px at 6 pt), and a tracked span anywhere else widened none. The canvas
path was correct on the same document — because `loki-layout` is on **parley
0.10**, where the two lines are present, while the Blitz stack is still on 0.6.
So the two paths disagreed for a reason visible in neither.

Bumping Blitz instead is not a two-line change: `blitz-dom`, `blitz-shell` **and
`blitz-paint`** share parley 0.6 types, and `blitz-paint` is not vendored here.

**Scope.** The `[patch.crates-io]` entry carries `version = "0.6.0"`, which
satisfies the Blitz crates' `^0.6` and leaves `loki-layout`'s `^0.10` resolving
from the registry. `cargo metadata` shows both, which is the check that this
stayed narrow.

**Removal condition:** the Blitz stack moves to parley ≥ 0.10. Watch for
`warning: Patch ... was not used` — if the Blitz crates' requirement moves off
`^0.6` the patch stops applying **silently as far as behaviour goes**, and the
defect returns; the line-break comparison
(`scripts/sitting/run.sh styledlinebreak` with `LB_FIXTURE=mixed:spacing`) is
what notices.

**Added:** 2026-08-05.

---

### blitz-dom — 0.2.4

**Source:** `patches/blitz-dom/` (local).

**Fixes:**

1. **Click-to-focus for non-input elements.** Upstream `handle_click` walks up
   the DOM but calls `clear_focus()` for any element that isn't
   input/label/a — clicking a wgpu canvas cleared keyboard focus from the
   nearest `tabindex="0"` ancestor, preventing `onkeydown` from firing. The
   patch checks `is_focussable()` and calls `set_focus_to()` instead.

   **Moved to mousedown 2026-08-02 (r79).** The focus assignment itself now
   happens in `handle_mousedown` (`focus_from_pointer`), not in `handle_click`.
   The driver runs the embedder's handler *before* blitz's default action, so
   focus assigned during `handle_click` lands **after** anything that handler
   did — including mounting an `autofocus` element. Every "click a button, a
   menu opens" interaction was therefore broken in the same way: the menu did
   receive focus, and the trigger took it straight back, leaving a raised
   overlay whose every key went to the button behind it. Browsers focus on
   mousedown for precisely this ordering reason, so this is the platform
   behaviour rather than a workaround for it. `handle_click` keeps the ancestor
   walk (it still decides where a default action applies) and the `label`
   branch focuses its bound input explicitly, since mousedown hit the label.

2. **Enter and Space activate a focused control (PATCH(loki), 2026-08-02).**
   `handle_keypress` did nothing at all for anything that was not a text input,
   so a `button` could be reached by Tab and never pressed — **every control in
   an embedder's UI was pointer-only**, WCAG 2.1.1. Tab traversal already
   worked, which is what made the gap hard to see: focus visibly moves, and then
   the keyboard stops.

   The patch dispatches a synthetic click `DomEvent` for Enter and Space on a
   focused non-text element, so the handler written for the pointer is the
   handler the keyboard runs. It goes through `dispatch_event`, **not** through
   `handle_click`: the latter is the *default action* for a click the embedder
   has already been told about, so calling it directly runs the built-in
   behaviour while the embedder's own `onclick` never fires — which is the whole
   point in a Dioxus app, where every button's behaviour lives in an `onclick`.

3. **`tabindex="-1"` is focusable but not tabbable (PATCH(loki), 2026-08-02).**
   `is_focussable` answered the tab-order question — `Some(index) => index >= 0`
   — so `tabindex="-1"` came out `false` and every consumer that meant "can this
   hold focus at all" got the wrong answer. That is exactly the combination HTML
   defines for a programmatically-focused container (an overlay, a dialog, a
   menu: focus it on open, never land on it while Tabbing past), so `autofocus`
   silently did nothing on any such element and the overlay's own key handler
   never saw a key.

   Split into two flags, because they are two questions: `is_focussable` for
   "may hold focus" and `is_tab_focussable` for "is in the sequential navigation
   order". `focus_next_node` takes the narrower one; `autofocus`, click-focus
   and the embedder's programmatic focus take the wider one.

4. **Scroll-change collection (PATCH(loki), 2026-06-10).**
   `scroll_node_by_collect` records each node whose scroll offset changed
   during a scroll gesture (including bubbling), and the `Document` trait
   gains a default-no-op `handle_scroll_changes` hook. blitz-shell calls the
   hook after wheel/touch scrolling; dioxus-native-dom implements it on
   `DioxusDocument` to dispatch DOM `scroll` events (with `NativeScrollData`
   payloads) into the VirtualDom, so Dioxus `onscroll` handlers fire.
   Routed through the `Document` trait because blitz-traits 0.2 has no
   scroll `DomEventData` variant.

5. **Absolute scroll (PATCH(loki), 2026-06-11).** `scroll_node_to_collect`
   scrolls a node to an absolute `(x, y)` offset (clamped, change-collecting),
   implemented on top of `scroll_node_by_collect`. Backs `MountedData::scroll`
   in the dioxus-native patch (draggable scrollbar thumb, scroll-to-cursor).

6. **Scroll-container enumeration (PATCH(loki), 2026-06-12).**
   `collect_scroll_containers` returns every node whose computed overflow is
   `scroll`/`auto`. blitz-shell calls it after a viewport resize (and after a
   scroll container mounts) and feeds the result to `handle_scroll_changes`, so
   the embedder re-receives `onscroll` with the new client size — letting the
   reflow view relayout to the window width without a user scroll.

7. **Non-viewport-bubbling scroll (PATCH(loki), 2026-06-20).**
   `scroll_node_within_collect` mirrors `scroll_node_by_collect` but drops any
   scroll that bubbles past the root element instead of moving the viewport
   (both delegate to a shared `scroll_node_by_collect_inner` taking a
   `bubble_to_viewport` flag). blitz-shell's wheel and touch handlers use it so
   the fixed full-window Loki shell never scrolls as a whole — a gesture that
   overruns the document, or starts over the ribbon, does nothing rather than
   jiggling the UI by the sub-pixel root/window slack.

8. **Static canvases don't force a per-frame redraw (PATCH(loki), 2026-06-21).**
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

9. **Embedder-supplied font blobs (PATCH(loki), 2026-06-27).** `DocumentConfig`
   gains `extra_fonts: Vec<Vec<u8>>`; `BaseDocument::new` registers each blob into
   the parley `FontContext` (on top of the system fonts and the default bullet
   font) at construction. This lets an app bundle its UI/fallback fonts and have
   them resolve **synchronously** on every platform, instead of relying on the
   asynchronous `@font-face` `data:` URI resource-fetch path (which did not load
   the UI typeface on Android). `dioxus-native`'s `Config::with_fonts(..)` feeds
   this field; the Loki apps pass `loki_fonts::ui_font_blobs()`.

10. **Wheel-gesture hook (PATCH(loki), 2026-08-02).** `Document::handle_wheel`
    reports one `WheelGesture` (node, delta, `WheelUnit`, pointer position,
    modifiers) to the embedder; the default is a no-op. A hook rather than a
    `DomEventData` variant for the same reason `handle_scroll_changes` is one —
    blitz-traits 0.2 has no wheel variant, and adding one means vendoring
    blitz-traits, which cascades into blitz-dom, blitz-paint and blitz-shell:
    four forks for one event.

    The delta and its unit travel together in `WheelGesture` because there is no
    honest conversion between them — turning lines into pixels needs a line
    height, and picking one fabricates a number the platform never gave.

11. **Scrollport geometry (PATCH(loki), 2026-08-02).**
    `BaseDocument::scrollport_origin` returns the origin of the innermost
    scrolling ancestor of a node, and `Node::border_box_position` returns a
    node's own box origin — `absolute_position` subtracts the node's *own*
    scroll offset, which places the origin at the top of its scrolled content.

    Both exist because an embedder cannot compute either: a DOM event's
    `offsetX`/`offsetY` are relative to its *target*, and recovering the
    scrollport frame from them needs the target's position within the
    scrollport, which is exactly what the event does not carry. The
    scroll-container test is shared with `scroll_node_by_collect_inner` rather
    than copied — two answers to "what counts as a scroll container" agree
    everywhere except on `overflow: visible` on `html`/`body`, which scrolls
    despite the value that normally means it does not.

10. **Per-run OpenType features via `data-font-features` (PATCH(loki),
    2026-08-05).** Stylo 0.8 gates both `font-kerning` and
    `font-feature-settings` to the Gecko engine, so in this build neither
    property exists and `stylo_to_parley::style` sets Parley's `font_features`
    to an unconditional empty list — leaving the shaper's default, which is
    **kerning on**. A document renderer needs it off unless the document asks:
    Word's `w:kern` and ODF's `style:letter-kerning` both default to off, and
    `loki-layout` already disables the feature accordingly
    (`para_build.rs`, gap #23).

    The gap was measured, not assumed (ADR-0017 §5.6): the same string set by
    the two paths came out **0.13 % narrower** in the DOM for ordinary serif
    prose and **7.7 % narrower** for a kern-heavy one, which moves a line break
    by several words. `build_inline_layout_recursive` now reads a
    `data-font-features` attribute off the inline element and passes it to
    Parley as `FontSettings::Source`; `dom_reflow::style::span_font_features`
    emits it. After the patch every case agrees to the pixel the box is rounded
    to.

    Attribute rather than CSS **because there is no CSS to use** — a
    `font-kerning` declaration is dropped as an unknown property, which is worse
    than nothing since it reads like a fix.

12. **Dead store removed in `layout/construct.rs` (PATCH(loki), 2026-08-15).**
    Not a behaviour change and not a fix for anything Loki needs — the *only*
    reason it is here is that upstream's `*anonymous_block_id = None` inside the
    whitespace-only branch is immediately followed by the same unconditional
    assignment, so every build of the workspace printed an `unused_assignments`
    warning from a file no consumer reads. A comment marks the spot. Drop this
    delta on the next rebase if upstream has removed the store itself.

**Removal condition:** Upstream blitz-dom implements tabindex focus-on-click
for non-input elements, dispatches scroll events to embedders, exposes an
absolute node-scroll API, stops treating a static canvas as perpetually
animating (e.g. a per-source "needs animation" signal), and Stylo exposes
`font-feature-settings` to the servo engine (at which point the reflow view
emits the CSS property and the `data-font-features` read goes away).

**Added:** 2026-05-18 (focus); extended 2026-06-10 (scroll events),
2026-06-11 (absolute scroll), 2026-06-21 (`needs_animation_tick` — stop the
idle canvas redraw loop, paired with the blitz-shell `redraw()` change),
2026-06-27 (`extra_fonts` — synchronous bundled-font registration),
2026-08-02 (`handle_wheel` + scrollport geometry), and 2026-08-05
(`data-font-features` — per-run kerning), together with matching changes in the
blitz-shell and dioxus-native(-dom) patches.

---

## Documented stack deviations (not patches)

Behaviours where this Blitz stack differs from the specification a reader would
otherwise assume. Not patched — either because the deviation is upstream's to
fix, or because working around it locally would be worse than knowing about it.
Recorded here because the failure mode is always the same: a call site that
reads correctly against the spec and is wrong against the implementation, which
no amount of care at the call site can catch.

**Three of these have been found one at a time, each by a consumer walking into
it rather than by anyone reading a list** — `position: fixed`, the coordinate
swap, and the absent `mouseleave`. Each cost a debugging session. The list is the
artifact rather than the individual entries: **before writing against a DOM
behaviour in this stack, read this section.** The next consumer of mouse events
will otherwise make the same inference from the same specification.

### `mouseenter` / `mouseleave` are not dispatched, and CSS `:hover` does nothing

**What the DOM guarantees:** a pointer entering and leaving an element produces
`mouseenter`/`mouseleave`, and `:hover` styles apply for the duration.

**What this stack does:** neither. Blitz dispatches no enter/leave pair and
honours no `:hover` rule, so an element cannot learn that the pointer has left
it.

**The consequence, and the shape of the workaround:** hover state has to be
tracked positively from `onmousemove` on each candidate element — entering a row
sets its key — and cleared by a *sibling* that covers the area outside them. The
spelling menu does exactly this, and it is why
`PopoverRequest::on_outside_move` exists: when the menu's own backdrop moved to
the popover host (r64), the clear signal had to move with it or the row tint
would have stuck to whichever row the pointer last crossed.

**Watch for:** any control whose appearance depends on the pointer being over it.
It will look correct while the pointer is moving and wrong the moment it stops
somewhere else, which reads as a repaint bug rather than a missing event.

### `clientX`/`clientY` are page coordinates, and `pageX`/`pageY` are missing

**What the DOM guarantees:** `clientX/clientY` are **viewport**-relative and
exclude scroll; `pageX/pageY` are **document**-relative and include it.

**What this stack does:** `blitz-dom/src/events/driver.rs` builds the mouse
event as

```rust
UiEvent::MouseDown(data) => DomEventData::MouseDown(BlitzMouseButtonEvent {
    x: data.x + viewport_scroll.x as f32 / zoom,
    y: data.y + viewport_scroll.y as f32 / zoom,
```

— winit's window-relative cursor position **plus the viewport scroll** — and
`dioxus-native-dom/src/events.rs` returns that verbatim from
`client_coordinates()`. That is the DOM's `pageX/pageY`. Meanwhile
`page_coordinates()` is `unimplemented!()`.

**So the two are swapped, not approximated.** A call site reading `client_x` gets
the opposite of the guarantee it is relying on, and the error is invisible
wherever scroll happens to be zero — which is most of the time, and all of the
time in a fixture.

**Same class as `position: fixed` collapsing to `absolute`** (see
`appthere-ui/src/components/overlay.rs`): a spec-conformant reading of the call
site is wrong, and only the source settles it.

**Consequence for anyone consuming mouse coordinates:** the value is
window-relative plus *top-level* scroll. **Inner scroll-container scroll is not
included.** Today that is harmless in `loki-text` — the app root is `100vh` with
`overflow: hidden` so top-level scroll is always zero, and the spelling menu's
containing block is the editor root, so an inner scroll moves anchor and
containing block together. It stops being harmless the moment a consumer's
containing block is *not* the anchor's scroll parent — which is exactly what
Spec 08 T4.1's root-hosted popover does.

**Found:** 2026-07-27, tracing Spec 08 T4.1's coordinate-space question.
**Upstream status:** not filed. **Removal condition:** a Blitz release where
`client_coordinates()` excludes scroll and `page_coordinates()` is implemented.

---

## Active patch — anyrender_vello (0.6.2)

*(A live `[patch.crates-io]` entry — kept as its own `##` section so it is not
filed under "Documented stack deviations (not patches)" above, which it is not.)*

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
   applied to the workspace's own Vello renderer construction in
   `loki-renderer/src/vello_init.rs`, and to the AA method each page tile
   requests in `loki-renderer/src/page_paint_source.rs`
   (COMPAT(android-mali) comments).

   *As of 2026-08-15 `vello_init.rs` has one caller — the standalone
   `PageSource` impl in `page_source_impl.rs`. Page tiles no longer construct a
   renderer at all; they borrow Blitz's, which carries these same Android
   settings. See "`CustomPaintCtx::renderer_mut`" below, including the AA
   coupling that now spans the two crates.*

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

**Additional fix (a custom source may return a smaller texture than its box):**
`VelloScenePainter::fill` now compares the texture a custom paint source
returned against the dimensions it was asked for, and scales the image brush to
compensate when they differ.

- *Root cause:* `render_custom_source` wraps the returned texture in an
  `ImageBrush` and `fill` passes the caller's `brush_transform` through
  untouched — which `blitz-paint`'s `draw_canvas` leaves `None`. The image is
  therefore sampled 1:1 against a rect sized from the element's content box, so
  a texture smaller than that box lands in the top-left corner with the rest of
  the box showing the brush's extend mode. Not a scaled-down page; a broken one.
- *Loki consumer:* Spec 08 T2.2. The resident-texture budget reduces the
  rasterisation scale of off-centre pages under memory pressure
  (`appthere_canvas::residency::plan_residency`), which is exactly this case —
  the tile keeps its on-screen box and its texture shrinks. Without the patch
  the budget's only remaining lever would be evicting visible content, which
  ADR L08-002 forbids.
- *Inert without a consumer:* the branch only fires when a source returns a
  texture of a different size from the one requested, which no upstream source
  does and which Loki itself does not do until the budget binds.
- *Upstream status:* candidate upstream fix — the custom-paint API already lets
  a source return any texture it likes, so the compositing side arguably has to
  handle the size mismatch rather than silently mis-sampling it.
- *Removal condition:* an anyrender_vello release that either scales a
  mismatched custom-paint texture itself or forwards the source's own brush
  transform.
- *Tested:* `scene_brush_fit_tests.rs`, against `fit_brush_to_box` — the
  arithmetic extracted from `fill` so it can be asserted without a GPU. The
  assertions are **point mappings, not scale factors**: brush space is texture
  space for an image brush, so "the texture covers its box" is "the transform
  carries `(0,0)` to the box origin and `(got.w, got.h)` to the box's far
  corner". Correct factors composed on the wrong side still read as `sx = 0.5`
  while placing the texture elsewhere, and the corner is what a reader would
  see. Verified discriminating by mutation: dropping the correction, swapping
  `pre_scale` for `then_scale`, and using one factor for both axes each fail
  it.
- **Sub-pixel correctness is still open**, and only that. Filtering choice and
  half-texel edge offsets need pixels; the gross case no longer does.

  *Previously this bullet read "the arithmetic and the policy that drives it
  are unit-tested headlessly". Only the policy was — `plan_residency` had
  tests, `fill`'s correction had none. The claim was corrected on 2026-07-27
  when the tests above were written.*

**Updated:** 2026-07-27

**Additional change (`CustomPaintCtx::renderer_mut` — a custom source may render
on the window's renderer):** `CustomPaintCtx` gained
`pub fn renderer_mut(&mut self) -> &mut VelloRenderer`, exposing the renderer it
already holds.

- *Why:* every `vello::Renderer` allocates a fixed set of GPU scratch buffers at
  construction — `lines` 48 MiB, `segments` 48 MiB, `ptcl` 32 MiB, `tiles` and
  `seg_counts` 16 MiB each, `blend_spill` 4 MiB, `bin_data` 1 MiB = **165 MiB**
  (`vello_encoding-0.6.0/src/config.rs`, `BufferSizes::new`). Those sizes are
  scene-**independent**; upstream's own comment says they were "hand picked to
  accommodate the vello test scenes as well as paris-30k" and "should instead get
  derived from the scene layout". A word-processor page needs a small fraction of
  that, but a second `Renderer` pays all of it, for the life of the process, from
  the first tile paint onward.
- *Loki consumer:* `LokiPageSource` (`loki-renderer/src/page_paint_source.rs`)
  used to build its own renderer in `resume()` — one per document, shared across
  that document's tiles — via `loki-renderer/src/vello_init.rs`. It now borrows
  Blitz's in `render()` through `ctx.renderer_mut()` and builds none.
  `RendererState::shared_renderer` and the `Arc<Mutex<Option<vello::Renderer>>>`
  threaded through `PageTileProps` were removed with it.
- *Measured* (macOS, 8 GB, `loki-text-desktop`, 24 KB DOCX, release):
  480 MB → **307 MB** physical footprint with a document open, peak 586 MB →
  417 MB. GPU-owned memory 365 MB → 200 MB — a 165 MB drop, matching the
  arithmetic above exactly. Launch with no document is unchanged (273 MB): no
  tile has painted, so the second renderer did not exist yet either.
- *Why it is sound to render here:* `VelloWindowRenderer::render` invokes custom
  paint sources from inside `draw_fn`, which returns **before** the window's own
  `render_to_texture`. There is no open render pass to nest inside, and
  `render_to_texture` is a self-contained encode-and-submit, so a tile render is
  sequenced ahead of the window render rather than re-entering it.
- **Coupling this introduces — the borrowed renderer must have compiled the AA
  variant the tile asks for.** A `vello::Renderer` only compiles the pipelines its
  `AaSupport` names. Today the two sides agree on both platforms by accident of
  matching COMPAT flags: desktop compiles `AaSupport::all()` and tiles request
  `AaConfig::Msaa16`; Android compiles `area_only()` and tiles request
  `AaConfig::Area`. Changing either side alone breaks tile rendering at runtime,
  and the two live in different crates (`patches/anyrender_vello/src/window_renderer.rs`
  and `page_paint_source.rs`). Nothing enforces the agreement yet — see the
  removal condition.
- *Upstream status:* candidate upstream fix. The custom-paint API already hands a
  source a `&mut VelloRenderer` internally; withholding it forces every source
  that renders its own scenes into a second 165 MiB allocation, which no source
  wants.
- *Removal condition:* an anyrender_vello release that either exposes the window
  renderer to custom paint sources or offers a render-scene-to-texture call on
  `CustomPaintCtx`. The AA coupling above should be closed first — ideally by
  having the ctx report its renderer's `AaSupport` (or take the `AaConfig` and
  reject an uncompiled one) rather than by a comment on each side.
- *Verified 2026-08-15 (macOS desktop, release, 24 KB / 14-page DOCX,
  `AppThere_Iris_Blueprint.docx`)* — window-ID screen capture (`screencapture
  -l<id>`, which reads the occluded window's own buffer) plus synthetic
  CGEvent scroll/click driving, one process throughout:
  - *Multi-tile lifecycle:* scroll page 1 → 14 → 1 (tiles mount/unmount and
    re-mount), zoom 100 % → 150 % → 600 % (crosses into reflow mode) → 25 %
    (three pages mounted at once) → 100 %, window resize 1440×870 → 900×700 →
    1440×870 (ribbon re-flows to its Compact posture), and minimise/restore.
    Every capture painted full page content — no blank, black, or stale tile,
    no error, warning, or panic in the log across the whole run.
  - *Pixel-level output:* page 1 at 100 % re-captured **after** all of the
    above is pixel-identical to its first render (`compare -metric AE`: the
    only differing components are the text caret at its old and new positions,
    the scrollbar, and two ribbon/status controls whose state changed —
    0 differing pixels anywhere else in the page raster). Two consecutive
    captures of one steady frame differ by 0 pixels, so the repaint itself is
    deterministic.
  - *Memory held:* GPU-owned footprint stayed at 198–218 MB through the churn
    (one renderer's 165 MB plus live tiles under the 92 MB tile budget), i.e.
    no second renderer appeared and no texture leak accumulated.
- *Still not verified:* Android on device — both the `area_only()` AA coupling
  and the real suspend/resume (macOS minimise does not tear the surface down
  the way an Android activity stop does).

**Updated:** 2026-08-15

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
   for f in Cargo.toml loki-renderer/Cargo.toml \
            loki-text/Cargo.toml loki-presentation/Cargo.toml loki-spreadsheet/Cargo.toml; do
     sed -i "s/version = \"=$OLD\"/version = \"=$NEW\"/" "$f"
   done
   ```

   (`appthere-canvas` declares no `dioxus` dependency, so it is not in the list;
   `appthere-ui` uses `dioxus = { workspace = true }` and inherits the pin from
   the root `Cargo.toml` bump above — no per-file `sed` needed.)

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

10. **Update docs:** the two patch section headers and re-vendor dates here, and
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
