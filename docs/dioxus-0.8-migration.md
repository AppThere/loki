# Dioxus 0.8 migration — survey and plan

**Status:** exploratory. Nothing has been changed in the tree; this document is
the survey and the plan.

**Surveyed against:** `dioxus 0.8.0-alpha.1` (latest 0.8 alpha on crates.io as of
2026-08-16; `0.8.0-alpha.0` preceded it, and `0.7.10` was published after
`0.8.0-alpha.0`). Current pin: `=0.7.9`.

**Method.** Upstream `.crate` tarballs for both stacks were downloaded and
diffed directly (0.7.9/0.2.x → 0.8.0-alpha.1/0.3.0-beta.1), and each of the
seven local patches was diffed against its own upstream baseline to separate
"what Loki changed" from "what upstream changed underneath it". Everything
marked **Observed** below is read off those diffs. Everything else is marked
**Not established**, with the check that would settle it.

---

## 1. Verdict up front

This is not a version bump. Migrating to 0.8 means:

1. **Rewriting the entire GPU page-tile integration.** `CustomPaintSource` /
   `CustomPaintCtx` / `use_wgpu` — the API `loki-renderer` is built on — **do
   not exist in the 0.8 stack**. They are replaced by a differently-shaped
   `blitz_dom::Widget` custom-element API.
2. **Re-vendoring five patches against a near-rewritten Blitz.** `blitz-dom`
   changed 8 825 of ~10 511 source lines between 0.2.4 and 0.3.0-beta.1. Loki's
   574-line patch against it is a reimplementation, not a rebase.
3. **A simultaneous graphics-stack bump**: wgpu 26 → 29, vello 0.6 → 0.9,
   peniko 0.5 → 0.6, kurbo 0.12 → 0.13, winit 0.30 → 0.31-beta.2, taffy 0.9 →
   0.12, parley 0.6 → 0.10, markup5ever 0.35 → 0.39.

Against that, 0.8 **retires a large amount of Loki-specific patch surface**:
scroll, wheel, touch, pointer and `mounted` events all land upstream, along with
both currently-documented stack deviations. The migration is worth doing — the
question is sequencing, and the honest read is that it should not start until
0.8 is at least at release-candidate.

**Recommendation:** do the preparatory work now (§6 Phase 0), which is valuable
whether or not 0.8 ships on schedule, and hold the stack bump itself until
`dioxus-native` 0.8 reaches rc. The blocking unknown (§5, R-1) is whether the
165 MiB single-Renderer optimisation survives; that is worth resolving with
upstream *before* committing to the port, not after.

---

## 2. The dependency landscape

**Observed** — from the crates.io index and the crate manifests:

| Crate | today (0.7.9 stack) | 0.8.0-alpha.1 stack |
|---|---|---|
| `dioxus` | `=0.7.9` | `=0.8.0-alpha.1` |
| `dioxus-native`, `dioxus-native-dom` | 0.7.9 | 0.8.0-alpha.1 |
| `blitz-{dom,shell,net,paint,traits,html}` | 0.2.x | `=0.3.0-beta.1` (exact-pinned by dioxus-native) |
| `anyrender` | 0.6.x | 0.11 |
| `anyrender_vello` | 0.6.2 | 0.12 (now **optional**) |
| `winit` | 0.30.11 | `=0.31.0-beta.2` (+ new `winit-android` crate) |
| `wgpu` | 26 | 29 |
| `vello` | 0.6 | 0.9 |
| `peniko` / `kurbo` | 0.5 / 0.12 | 0.6 / 0.13.1 |
| `parley` (Blitz side) | 0.6 | **0.10** |
| `taffy` / `stylo_taffy` | 0.9 / 0.2 | 0.12.1 / `=0.3.0-beta.1` |
| `markup5ever` | 0.35 | 0.39 |
| `android-activity` | 0.6 | **0.6 — unchanged** |

Two consequences worth calling out separately:

- **`android-activity` does not move.** winit 0.31 factors Android into a
  `winit-android` crate, which still requires `android-activity ^0.6.0` and
  `ndk ^0.9.0`. The `android_main!` FFI entry point and the unsafe-policy
  allowlist are unaffected.
- **The default renderer changed.** `dioxus-native` 0.8's default feature set is
  `vello-hybrid`, not `vello`, and `autofocus` is no longer defaulted. Loki
  needs full `vello` for the texture path. Since the `dioxus` meta-crate's
  `native` feature only does `dep:dioxus-native` with defaults, **Loki should
  take a direct `dioxus-native` dependency** with `default-features = false` and
  an explicit feature list. That alone retires one of the two reasons the
  `dioxus-native` patch exists (§4).

---

## 3. What breaks, by area

### 3.1 GPU page tiles — the big one

**Observed.** `dioxus-native` 0.7.9 re-exports
`anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle}`
and `dioxus_renderer::use_wgpu`. In 0.8.0-alpha.1 **none of these exist**;
`anyrender_vello` 0.12 has no `custom_paint_source.rs` at all.

The replacement is a custom-widget API:

- `blitz_dom::Widget` — `connected` / `disconnected` / `attribute_changed` /
  `can_create_surfaces(&mut dyn RenderContext)` / `destroy_surfaces` /
  `handle_event(&UiEvent)` / `paint(&mut dyn RenderContext, &ComputedStyles, w, h, scale) -> Scene`.
- Attached from RSX via `dioxus_native_dom::CustomWidgetAttr::new(widget)` set as
  the `data` attribute of an `<object>` element (`mutation_writer.rs:223-237`).
- A widget renders to wgpu by taking the `DeviceHandle` out of
  `RenderContext::renderer_specific_context()` (a `Box<dyn Any>` downcast),
  creating its **own** `wgpu::Texture`, registering it with
  `RenderContext::try_register_custom_resource` → `ResourceId`, and drawing that
  id as an image into the returned `Scene`.

Files affected (`loki-renderer`, ~1 000+ lines): `page_paint_source.rs`,
`page_paint_lifecycle.rs`, `page_paint_render.rs`, `page_tile.rs`,
`renderer_state.rs`, `doc_page_source.rs`, `doc_page_source_scale.rs`,
`dpr_probe.rs`, `gpu_probe.rs`, plus `page_source_impl.rs` and the
`appthere-canvas` residency accounting that hangs off the texture lifecycle.

Mapping notes:

- `resume(&DeviceHandle)` → `can_create_surfaces(&mut dyn RenderContext)` +
  downcast. Loki's own upstreamed-in-patch `release(ctx)` hook maps cleanly onto
  `destroy_surfaces` / `disconnected` + `RenderContext::unregister_resource`, so
  the texture-leak fix that hook exists for is now expressible without a patch.
- The `scale` argument survives (`paint(..., scale: f64)`), so `dpr_probe`'s
  one-frame-lagged DPR read has an equivalent landing site.
- `paint` now also receives `&ComputedStyles`, which is strictly more than
  today's `render` gets.

### 3.2 Events — mostly *fixed* upstream

**Observed.** `blitz-traits` 0.3 `DomEventData` grows from 9 variants to ~30,
adding `Scroll`, `Wheel`, `TouchStart/Move/End/Cancel`, the full `Pointer*` set,
`MouseEnter/Leave/Over/Out`, `ContextMenu`, `DoubleClick`, `FocusIn/Out`.
`dioxus-native-dom` 0.8 implements `convert_scroll_data`, `convert_wheel_data`,
`convert_touch_data`, `convert_pointer_data` and `convert_mounted_data`, and
ships a `NodeHandle: RenderedElementBacking` with `scroll`, `scroll_to`,
`get_scroll_offset`, `get_scroll_size`, `get_client_rect` and `set_focus`.
Dispatch walks the ancestor chain to the nearest Dioxus-mapped node
(`dioxus_document.rs:334-337`) — precisely the bubbling behaviour Loki's wheel
patch added.

That is essentially the whole of `patches/dioxus-native-dom` (504 changed lines)
plus the touch-forwarding half of `patches/blitz-shell`, landing upstream.

**Both documented stack deviations in `docs/patches.md` also close:**
`mouseenter`/`mouseleave` now exist as event variants, and `PointerCoords`
carries `page_x/page_y` **and** `client_x/client_y` separately — so the
"`clientX`/`clientY` are page coordinates and `pageX`/`pageY` are missing"
deviation is resolved at the source.

**The one residual gap.** Loki's `NativeWheelData` carries a
**scrollport-relative** position, consumed by
`loki-text/src/routes/editor/editor_wheel_zoom.rs:153` (`data.scrollport`) to
anchor Ctrl+wheel zoom. Upstream's `BlitzWheelEvent` carries `coords`
(page/client) and `element` (target-relative) only, and the scrollport frame
cannot be derived from those by the consumer. This needs either a small
retained patch or re-derivation from `get_client_rect` on the scroll container.

**Still `unimplemented!()` in 0.8**: composition (IME), clipboard, selection,
drag, animation, media, image, transition, resize, visible, cancel,
beforeinput. Composition was never implemented in Loki's patch either, so this
is not a regression — Android IME continues to depend on
`patches/blitz-shell/src/ime_android.rs`.

### 3.3 Launch / config API

**Observed.** `Config::with_fonts(Vec<Vec<u8>>)` — used by
`loki-text/src/main.rs:65` — is a **Loki patch method**, not upstream. 0.8 ships
`Config::with_font_ctx(FontContext)` plus `blitz_dom::build_single_font_ctx` for
the one-font case. Loki registers many blobs (`loki_fonts::ui_font_blobs()`), so
it must build a multi-font `FontContext` and pass that instead.

`winit` 0.31 renames apply to the launch path as well
(`WindowAttributes::with_inner_size` → `with_surface_size`; `Window` is now
`Arc<dyn Window>`). Affects `loki-text/src/main.rs`, the `loki-spreadsheet` and
`loki-presentation` entry points, and `appthere-ui/examples/{linebreak,nested_scroll}_probe.rs`.

New and useful: `use_window_event` and `use_back_button` hooks — the latter is
the Android hardware back button, which Loki currently has no first-class path
for.

### 3.4 Graphics crates

`loki-vello` (`vello 0.6`, `peniko 0.5`, `kurbo 0.12`, `wgpu 26`),
`appthere-canvas` (`wgpu 26`, `peniko 0.5`), `loki-renderer` (`wgpu 26`,
`vello 0.6`, `anyrender_vello 0.6.2`) and `loki-graphics` must all move in
lockstep to wgpu 29 / vello 0.9 / peniko 0.6 / kurbo 0.13, because the tile
textures are handed to Blitz's renderer and both sides must agree on the wgpu
version.

**Not established:** the size of the vello 0.6 → 0.9 API delta for the surface
Loki actually uses (`Renderer::new`, `render_to_texture`, `RenderParams`,
`AaConfig`, `Scene`). *What would settle it:* a scratch crate compiling
`loki-vello` against vello 0.9 in isolation — it has no Blitz dependency, so
this can be done today, before any other migration work.

### 3.5 Layout and CSS

taffy 0.9 → 0.12.1, `stylo_taffy` 0.2 → 0.3, a Stylo bump, and markup5ever
0.35 → 0.39 all land at once. `dioxus-native` 0.8 additionally gains `floats`
and `incremental` features (the latter now on by default).

**Not established:** whether the "Confirmed CSS properties" list in `CLAUDE.md`
still holds — in particular `position: absolute` (which the floating spelling
context menu depends on, and which was only confirmed working on the *current*
Stylo + stylo_taffy 0.2 + Taffy 0.9 stack) and whether `position: fixed` still
collapses to `absolute`. *What would settle it:* re-run the existing runtime
probes (`appthere-ui/examples/*_probe.rs`) against the 0.8 stack and re-record
the results in `CLAUDE.md` and `docs/fidelity-status.md`.

### 3.6 Dioxus core / macros

- `#[component]`-derived props structs are now `#[non_exhaustive]`. Loki's
  hand-written `#[derive(Props)]` structs are unaffected; any *construction* of
  a macro-derived props struct by literal would break. Grep shows none, but this
  is worth a compile-driven check rather than a grep.
- Stores/signals: `ReadStore` now requires `Lens: Readable`; store derives
  enforce method/field visibility. Loki uses `Signal`/`WritableExt` but no
  `dioxus-stores` derives, so exposure looks low.
- Dioxus moved to edition 2024. Loki is already predominantly edition 2024
  (~46 crates) with a handful still on 2021; the pinned 1.97.1 toolchain
  supports it either way.

---

## 4. Patch disposition

Seven `[patch]` entries touch this stack. `loki-file-access` and
`appthere-color` are unrelated and unaffected.

| Patch | Loki's delta | Upstream churn underneath | Disposition |
|---|---|---|---|
| **`parley` 0.6.0** | 2 lines | — | **Drop.** blitz-dom 0.3 uses parley **0.10**, which carries the letter/word-spacing fix. This also collapses the dual-shaper deviation: Blitz and `loki-layout` would run the *same* parley version for the first time. |
| **`dioxus-native-dom`** | 504 lines | 690 of 924 lines | **Mostly drop.** Scroll/wheel/touch/pointer/mounted all upstream. Retain only: `NativeWheelData.scrollport` (§3.2), and re-verify the onmounted pending-drain and the focus-preservation-across-render behaviour against 0.8's own implementations. |
| **`dioxus-native`** | 281 lines | 442 of 674 lines | **Mostly drop.** `with_fonts` → upstream `with_font_ctx`; `autofocus` → feature selection on a direct dependency. Retain only if the Android `request_redraw`-after-`CreateHeadElement` fix is still needed — 0.8's incremental-rendering redraw path may already cover it. |
| **`blitz-shell`** | 836 lines (incl. whole files `ime_android.rs`, `tooltip.rs`) | 1 222 of 1 514 lines (~81 %) | **Re-implement.** Touch forwarding is obsolete (winit 0.31 pointer events, `convert_events.rs:69-107`). `ime_android.rs` and `tooltip.rs` are self-contained subsystems that must be re-vendored against the rewritten `window.rs`. |
| **`blitz-dom`** | 574 lines across 10 files | **8 825 of 10 511 lines (~84 %)** | **Re-implement from scratch.** `scroll_node_by` now exists upstream with a dispatch callback. Still absent upstream: `scrollport_origin`, `is_tab_focussable` (tab order ≠ focusability), `scroll_axes`, the accesskit stale-focus guard, and the focus-on-click fix — `handle_click` still calls `clear_focus()` on no match (`events/pointer.rs:742`). |
| **`blitz-net`** | manifest-only (0 src lines) | 346 lines | **Re-decide.** 0.3 still defaults to `native-tls`, but now adds a `cfg(target_os = "android")` override using `native-tls-vendored` (vendors OpenSSL rather than linking `libssl.so`). Loki's rustls override is still expressible as a manifest-only patch; whether it is still *needed* depends on whether the vendored build is acceptable for APK size. |
| **`anyrender_vello`** | 229 lines | 408 of 560 lines | **Split.** The Android Mali `num_init_threads = 1` workaround is **still needed** — 0.12 still applies it for macOS only (`lib.rs:18-21`). `release(ctx)` is superseded by the `Widget` lifecycle. `renderer_mut()` has **no 0.8 home** — see R-1. |

---

## 5. Risks and open questions

**R-1 — the 165 MiB Renderer-sharing optimisation has no 0.8 equivalent.**
*Observed:* commit `9de9a8d` moved page tiles onto Blitz's own `vello::Renderer`
via a Loki-added `CustomPaintCtx::renderer_mut()`, worth ~165 MiB (a second
`vello::Renderer` allocates fixed-size scratch buffers regardless of scene). In
0.8, `VelloScenePainter.renderer` is `pub(crate)`, and
`RenderContext::renderer_specific_context()` returns **only** a cloned
`DeviceHandle` — the `Renderer` is not reachable from a `Widget`.
*Not established:* whether upstream would accept exposing it, and whether the
`Widget` texture path re-introduces the second-Renderer cost by construction.
*What would settle it:* raise it with Blitz/Dioxus upstream **before** starting
the port — this is the single question most likely to change the shape of the
`loki-renderer` rewrite. Second-best: prototype a `Widget` that registers a
texture and measure resident GPU memory against today's baseline.

**R-2 — 0.3.0-beta.1 is exact-pinned by `dioxus-native`.** Every Blitz crate is
`=0.3.0-beta.1`. Loki's own direct `blitz-dom`/`blitz-html`/`blitz-traits`/
`blitz-shell` dependencies (in `loki-text`, `loki-presentation`,
`loki-spreadsheet`) must be pinned to the identical version or the graph
duplicates and the patches silently stop applying — the same failure mode the
existing `=0.7.9` pin exists to prevent.

**R-3 — alpha churn.** Between `0.8.0-alpha.0` and `0.8.0-alpha.1` the Blitz pin
moved `0.3.0-alpha.4` → `0.3.0-beta.1` and every anyrender crate bumped a major.
Re-vendoring 1 400+ lines of patch against a moving target is the dominant cost
of migrating early rather than at rc.

**R-4 — CSS/layout regressions are invisible until run.** §3.5. The confirmed-CSS
list and `docs/fidelity-status.md` are the affected records.

**R-5 — the ACID fidelity harness is the only real acceptance gate, and half of
it is empty.** Whether `loki-acid`'s page-count/glyph-coverage canaries still
pass after a parley 0.6 → 0.10 shaper change on the Blitz side is unknown and
cannot be predicted from source reading. The SSIM half is worse than unknown:
§8 (0.4) establishes that it asserts nothing today, because no goldens exist.
Populating them is a Phase 0 prerequisite, not a Phase 3 activity.

---

## 6. Plan

### Phase 0 — preparation (can start now, no 0.8 dependency)

Everything here is useful even if 0.8 slips, and each item shrinks the eventual
port. **Status is tracked in §8** — two of the five items turned out to be
mis-scoped when attempted, and §8 records what replaced them.

- **0.1** Take a **direct `dioxus-native` dependency** for feature selection
  instead of relying on `dioxus/native` defaults. Retires the `autofocus`
  half of the `dioxus-native` patch today.
- **0.2** Establish the vello 0.6 → 0.9 / wgpu 26 → 29 / peniko 0.5 → 0.6 /
  kurbo 0.12 → 0.13 API delta for the surface Loki actually uses, so §3.4 stops
  being an unknown.
- **0.3** Raise R-1 with upstream Blitz/Dioxus (expose the `Renderer` to custom
  widgets, or document the intended zero-copy path).
- **0.4** Write down the current runtime-probe baselines (CSS probes, texture
  residency at a known document + zoom, ACID canary results) so the post-migration
  comparison has a *floor as well as a ceiling* — a migration that reports
  "improved" memory is exactly the one nobody double-checks.
- **0.5** Re-derive `editor_wheel_zoom`'s scrollport anchor from public geometry
  (`get_client_rect` on the scroll container) if possible, removing the last
  reason to keep a `dioxus-native-dom` patch.

### Phase 1 — the stack bump (gated on 0.8 rc, or on R-1 being answered)

- **1.1** Bump the pin to `=0.8.0-rc.N` across the workspace; pin every direct
  Blitz dependency to the exact version `dioxus-native` requires (R-2).
- **1.2** Delete the `parley` patch. Confirm a single parley version resolves
  workspace-wide.
- **1.3** Re-vendor `anyrender_vello` carrying **only** the Android
  `num_init_threads` workaround (plus the image-brush-transform hunk if still
  needed).
- **1.4** Re-vendor `blitz-net` as a manifest-only rustls override, or drop it if
  upstream's `native-tls-vendored` Android path is acceptable on APK size.
- **1.5** Expect the workspace not to compile until Phase 2 lands. Track it as
  one branch, not a series of half-migrated commits.

### Phase 2 — the two rewrites

- **2.1 `loki-renderer` on `Widget`.** Port `LokiPageSource` to
  `blitz_dom::Widget`; move device acquisition to `can_create_surfaces`, texture
  registration to `try_register_custom_resource`, teardown to
  `destroy_surfaces`/`disconnected`. Replace `use_wgpu(...)` + canvas-id
  plumbing in `page_tile.rs` with `object { data: CustomWidgetAttr::new(...) }`.
  Keep `TileKey`, the residency budget, and the raster-permille concession
  intact — they are renderer-agnostic and should survive unchanged.
- **2.2 `blitz-dom` / `blitz-shell` patches, re-implemented.** Against ~84 % and
  ~81 % rewritten upstreams, port each Loki behaviour individually and re-justify
  it against the new code rather than re-applying hunks: focus-on-click,
  accesskit stale-focus, tab-order, scrollport geometry, `ime_android`, `tooltip`.
  Several may turn out to be unnecessary; that is a finding, not a shortcut.

### Phase 3 — patch retirement and re-verification

- **3.1** Delete the obsolete parts of `dioxus-native-dom` and `dioxus-native`,
  and the touch-forwarding part of `blitz-shell`. Update `docs/patches.md` in the
  same change — including moving both now-fixed stack deviations into the
  "Removed" section with the version that fixed them.
- **3.2** Re-run the CSS runtime probes; update the confirmed/unconfirmed lists
  in `CLAUDE.md` and `docs/fidelity-status.md`.
- **3.3** Run the full ACID harness and compare against the Phase 0.4 baselines,
  in both directions.
- **3.4** Re-verify on Android device: Mali shader init, IME, touch scroll and
  long-press selection, back button (now expressible via `use_back_button`),
  APK size.
- **3.5** `cargo fmt --all --check` and the exact CI clippy command.

### Not in scope

Adopting 0.8's new native APIs (SwiftUI/Kotlin widgets, portals, the FFI
system, `Config::floats`). Those are follow-on work; folding them into the port
would make the regression surface unattributable.

---

## 8. Phase 0 progress (2026-08-16)

### 0.1 — direct `dioxus-native` dependency · **done**

`dioxus-native = { version = "=0.7.9", features = ["autofocus"] }` is declared in
`[workspace.dependencies]` and taken by `appthere-ui` and `loki-text` — the two
crates whose RSX carries the `autofocus` attribute. `dioxus/native` still pulls
the same package with its default features; Cargo unions the two requirements,
so `autofocus` is on without touching the vendored manifest.

`autofocus` has been removed from `patches/dioxus-native/Cargo.toml`'s `default`
list, which is now upstream's, unmodified. `docs/patches.md` records the change.

Nothing imports `dioxus_native` — the dependency exists purely for feature
selection, and the comment at each site says so, because a future "remove unused
dependency" pass would otherwise silently return the editor canvas to
needs-a-click-before-typing with no build error to mark it.

### 0.2 — graphics-crate delta · **measured; the plan's premise was wrong**

**The claim that this was independently landable does not hold.** `loki-vello`
has no *Blitz* dependency, but it shares the `vello::Scene` **type** with
`loki-renderer`: `loki_vello::paint_single_page(scene: &mut vello::Scene, …)` is
called from `RenderLayout::paint_tile`, on a `Scene` that
`page_paint_source.rs:186` constructs and hands to Blitz's renderer. `loki-vello`
and `loki-renderer` must therefore agree on a vello version, and `loki-renderer`
must agree with `anyrender_vello` — which is on vello 0.6 until the whole Blitz
stack moves. Bumping `loki-vello` alone breaks the workspace.

So 0.2 was done as a measurement instead, from upstream source. The result is
much better than §3.4 assumed:

| Crate | Delta for Loki's surface |
|---|---|
| `peniko` 0.5.0 → 0.6.0 | **Zero** public symbol removals or renames. |
| `kurbo` 0.12.0 → 0.13.1 | **Zero** public symbol removals or renames. |
| `vello` 0.6.0 → 0.9.0 | `Scene` is **additive only** (`brush_transform`, `font_embolden` added; `push_*_layer` generics widened). `Renderer::new`, `render_to_texture`, `RenderParams`, `RendererOptions`, `AaConfig`, `AaSupport` are unchanged in shape. |
| `wgpu` 26 → 29 | The only real work — but Loki's whole wgpu surface is `Device`, `Queue`, `Texture`, `TextureDescriptor`, `TextureViewDescriptor`, `TextureFormat`, `TextureDimension`, `TextureUsages`, `Extent3d`, `Instance`, `DeviceType`. Texture creation and device handles, i.e. the most stable part of the API. |

*Instrument note:* the first symbol diff reported ~30 removals from kurbo,
including `Rect::min_x`. That was the instrument, not the library — the regex
did not match `pub const fn`, and kurbo 0.13 made those methods `const`. Re-run
with a `const`-aware pattern, both diffs are empty. Recorded because a
"30 removals" figure would have inflated this workstream's estimate on the
strength of a broken grep.

**Revised read:** §3.4 is not a risk area. Fold the graphics bump into Phase 1
as a single mechanical step, and drop it as a separate workstream.

### 0.5 — scrollport anchor · **reclassified to Phase 2, do not do this**

The intent was to remove Loki's last reason to patch `dioxus-native-dom` by
deriving the wheel-zoom anchor from public geometry instead of the patch-carried
`NativeWheelData.scrollport`.

It is *arithmetically* possible — `scrollport_x = client_x − client_rect.x`, and
the scroll container's `MountedData` is already captured (`editor_state.rs:134`).
It should still not be done:

1. **`get_client_rect` is async.** It returns
   `Pin<Box<dyn Future<Output = MountedResult<PixelsRect>>>>`, so it cannot be
   read inside a synchronous `onwheel` handler. The rect would have to be cached
   and invalidated on resize, zoom, and layout change.
2. **That is a second derivation of one fact** (rule 4). The patch computes the
   scrollport frame from live layout at dispatch time; a cached rect is the same
   fact, derived differently, and free to drift. A stale rect does not fail — it
   anchors the zoom to the wrong point, which reads as "zoom feels off" and is
   almost impossible to attribute.

The patch-carried value is the *better* engineering, and trading it away to
lower the patch count would be optimising the wrong number. In 0.8 this becomes
a genuinely small patch: `blitz-dom` already needs `scrollport_origin` for other
reasons (§4), so carrying the scrollport frame onto the wheel event costs one
extra field on top of a patch that has to exist anyway.

### 0.4 — baselines · **partly done, and it found a hole in the gate**

`loki-acid/examples/structural_baseline.rs` prints the structural numbers as a
stable, diffable table (`cargo run -p loki-acid --example structural_baseline`).
The canaries assert the numbers are *acceptable* — a ceiling; this records what
they actually are — a floor. Baseline on `1.97.1`, this container:

```
fixture                            pages sheets slides  glyphs  coverage
acid_docx.docx                        19      -      -    6115    1.0000
acid_odt.odt                           2      -      -    2179    1.0000
acid_xlsx.xlsx                         -      9      -       -         -
acid_pptx.pptx                         -      -      3       -         -
acid_ods.ods                           -      1      -       -         -
acid_odp.odp                           -      -      -       -         -
acid_odg.odg                           -      -      -       -         -
```

**The pixel gate does not currently exist.** R-5 called the ACID harness "the
only real acceptance gate" for the migration. That is only true of its
*structural* half:

- `golden_pixel::golden_pages_match_within_ssim_threshold` **passes in 0.00 s**.
  `loki-acid/goldens/` contains a `README.md` and nothing else, and
  `loki-acid/renders/` is empty, so the test iterates zero pages. It is green
  today and would be equally green after a rendering regression of any size —
  an instrument that cannot speak where the hazard is.
- `structural::no_tofu_glyphs` is `#[ignore]`d (host-font-dependent), so the
  strict tofu check does not run here either.

What *does* gate: import success, page/sheet/slide counts, and glyph coverage
(1.0000 on both paginated fixtures) — six passing canaries. Those would catch a
pagination or shaping collapse, which is the most likely parley 0.6 → 0.10
failure mode, but not a paint regression.

**Consequence for the plan:** populate `goldens/` before Phase 2, or accept
explicitly that the migration has no pixel-fidelity net and say so in the PR.
This is a prerequisite discovered by Phase 0, not an optional extra — and it is
worth doing while the *current* stack is still buildable, because goldens
captured after the migration prove nothing about it.

**Follow-up (same day): a pixel gate does exist — it is not `loki-acid`'s.**
See §10.

### 0.3 — outstanding

Needs an upstream conversation; the question to ask is drafted in §9 so it does
not have to be re-derived.

---

## 9. The question for upstream (R-1)

To be raised on the Blitz repo before Phase 2 starts. Stated here so it does not
get re-derived:

> In 0.7's `anyrender_vello`, a `CustomPaintSource` could be handed the window's
> `vello::Renderer` and record its own scenes onto it. In 0.8 the equivalent is a
> `blitz_dom::Widget`, and the only renderer-specific handle reachable from
> `RenderContext::renderer_specific_context()` is a `DeviceHandle`;
> `VelloScenePainter::renderer` is `pub(crate)`.
>
> A widget that renders a large GPU scene therefore has to construct its own
> `vello::Renderer`. That is a flat ~165 MiB — `vello_encoding::BufferSizes::new`
> allocates fixed-size scratch buffers sized for a stress scene, independent of
> what is actually drawn — on top of the one the window already owns.
>
> Is exposing the `Renderer` (or a scene-recording entry point) to custom widgets
> something you'd take a PR for, or is there an intended path for this that we've
> missed?

---

## 10. Goldens: what was found when we went to populate them (2026-08-16)

The instruction was "populate the goldens before we go further". The result is
better than expected in one direction and blocked in another.

### A working pixel gate already exists

`loki-render-cpu/tests/visual_golden.rs` compares Loki's deterministic
`vello_cpu` candidate render against committed LibreOffice goldens in
`appthere-conformance/goldens/odt/`, at a calibrated SSIM/ΔE tolerance, with no
GPU. **Three ODT fixtures, all passing, in 1.53 s** — a real gate, unlike
`loki-acid`'s 0.00 s no-op.

Its golden pipeline was verified rather than assumed: regenerating
`para-carlito` through `soffice --convert-to pdf` + `rasterize_pdf` reproduced
the committed PNG **bit-for-bit**. (Environment needed fixing first — this
container had `libreoffice-core` without `libreoffice-writer`, so *no* document
filter could load *any* file. A control conversion of an unrelated file failed
identically, which is what separated "broken install" from "bad fixture".)

### `loki-acid`'s golden tree is a duplicate, and populating it creates no gate

Its `renders/` side has no in-repo producer, so `golden_pixel` compares zero
pages regardless of how many goldens are added. `loki-acid/goldens/README.md`
now records this and points at the working harness. Two golden systems for one
fact is the drift this suite exists to catch.

### The acid fixtures cannot join the working gate as they stand

Measured with the new `loki-render-cpu/examples/measure_odf_golden` instrument:

| fixture | worst region | verdict |
|---|---|---|
| `styles-tinos.odt` (conformance, font-pinned) — control | ssim 0.6603, ΔE 7.854 | **passes** |
| `acid_odt.odt` page 1 | ssim 0.0845, ΔE 22.115 | fails |
| `acid_odt.odt` page 2 | ssim 0.2188, ΔE 14.753 | fails |

Page counts and dimensions agree, the import is clean, and Loki reports **no
font substitutions**. The cause is that `acid_odt.odt` declares no
`<style:default-style>`: its only font declaration sits on the named style
`BaseBody`, so paragraphs not inheriting from it fall back to each
*application's own* default face — Liberation Serif in LibreOffice, a sans face
in Loki. That repaints nearly every glyph, for a reason about the fixture rather
than either renderer. The conformance fixtures are font-pinned by name for
exactly this reason.

*Correction:* the first hypothesis here was a missing "Liberation Serif" alias in
`loki-layout`'s substitute table. That was wrong — `resolve_font_name` returns
`Liberation Serif` unchanged, and the substitution run is empty. Loki's own
instrument settled it; the pixels alone would not have.

### OOXML goldens are blocked by design, not by this environment

`acid_docx` / `acid_xlsx` / `acid_pptx` and the three pending
`appthere-conformance/goldens/docx/` fixtures require **Microsoft 365 desktop**,
which cannot be automated headlessly. Substituting LibreOffice would be actively
wrong: every `TC-DOCX-*` row in `loki-acid/TEST_PLAN.md` names LibreOffice's
divergence as the thing under test, so a LibreOffice "golden" would enshrine the
known-wrong render as the reference. These need a Windows/macOS capture via
`scripts/generate-office-goldens.sh`.

### But note what this gate does and does not cover for *this* migration

The CPU conformance path renders `loki_layout` output. `loki-layout` is on
**parley 0.10 already**, while the Blitz stack is on the patched 0.6 — the two
shapers are deliberately independent (root `Cargo.toml`). So the parley
0.6 → 0.10 change this migration brings **does not touch the document layout
path at all**; it affects Blitz-rendered surfaces: UI chrome and the
`dom_reflow` view.

That is the uncomfortable part: the gate that exists covers the path the
migration barely changes, and the path the migration *does* change — Blitz's own
CSS/paint — has no golden harness, because it is GPU-only and there is no GPU
here. Populating more goldens does not close that gap.

**Recommended next step**, in preference order:

1. Capture the Word goldens on a Windows/macOS box — unblocks the DOCX axis and
   is valuable independently of Dioxus 0.8.
2. Font-pin `acid_odt.odt` (add an explicit `<style:default-style>`), regenerate
   its golden, re-measure. Note `loki-render-cpu` still paints a grey placeholder
   for embedded images (`TODO(conformance-render)`), which caps the achievable
   score for image-bearing fixtures.
3. For the Blitz-side risk specifically, accept that the net is manual: the
   runtime CSS probes in §6 Phase 3.2 plus a device pass, not a pixel diff.

---

## 11. What this survey did not check

Stated plainly so the gaps are not mistaken for clean results:

- No compilation was attempted against 0.8 — every claim here is read off
  upstream source diffs, not off a build.
- The vello 0.6 → 0.9 and wgpu 26 → 29 API deltas were not enumerated (§3.4).
- `dioxus-router` 0.8's delta was not examined; `loki-text`,
  `loki-spreadsheet` and `loki-presentation` all enable the `router` feature.
- The Stylo/taffy CSS behaviour delta was not assessed beyond noting the version
  jump (§3.5).
- Whether `blitz-dom` 0.3's `flush_is_focussable` already covers the
  `tabindex="0"` div case that Loki's focus-on-click patch exists for was not
  determined — it is a source read away, but the answer only matters once 2.2
  starts.
