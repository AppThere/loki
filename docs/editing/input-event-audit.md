# Input Event Audit — Dioxus Native 0.7 / Blitz

## Date
2026-04-26

## Event routing architecture

```text
OS event
  → winit WindowEvent
    → blitz-shell/src/window.rs (View::handle_winit_event)
      → blitz-dom/src/document.rs (handle_ui_event)
        → Dioxus synthetic event (dioxus-native-dom/src/events.rs NativeConverter)
          → loki-text component handler
```

## Event availability table

| Event class | winit type | Blitz consumed? | Dioxus handler | Notes |
|-------------|-----------|-----------------|----------------|-------|
| Mouse click | `MouseInput` | No | `onclick` / `onmousedown` | Handled and forwarded as `UiEvent::MouseDown`/`Up`. Dioxus maps to `onclick`/`onmousedown`/`onmouseup`. |
| Mouse move | `CursorMoved` | No | `onmousemove` | Forwarded as `UiEvent::MouseMove`. |
| Mouse wheel | `MouseWheel` | YES | `onwheel` | Consumed strictly for scroll by `blitz-shell`. Does not fire Dioxus handlers. |
| Key down | `KeyboardInput` | Partially | `onkeydown` | Forwarded aside from some Ctrl/Alt shortcuts (zoom, devtools) which are consumed. |
| Key up | `KeyboardInput` | No | `onkeyup` | Forwarded. |
| Character input | `KeyboardInput` | No | `onkeydown` | Available as `text: Option<SmolStr>` on the keydown event. |
| IME composition start | `Ime::Preedit` | Internally | `oncompositionstart` | `UiEvent::Ime` forwarded to blitz, but Dioxus `convert_composition_data` is explicitly `unimplemented!`. |
| IME composition update | `Ime::Preedit` | Internally | `oncompositionupdate` | Same as above. |
| IME composition commit | `Ime::Commit` | Internally | `oncompositionend` | Same as above. |
| Touch start | `Touch` | YES | `ontouchstart` | Ignored intentionally in `window.rs` (`WindowEvent::Touch(_) => {}`). Doesn't reach Dioxus. |
| Touch move | `Touch` | YES | `ontouchmove` | Ignored. |
| Touch end | `Touch` | YES | `ontouchend` | Ignored. |
| Focus gained | `Focused` | YES | `onfocus` | `WindowEvent::Focused` is ignored by blitz-shell. Focus operates internally based on Tab index and clicks. |
| Focus lost | `Focused` | YES | `onblur` | Ignored from OS perspective. |

## Keyboard events

1. **Does `onkeydown` fire?** Yes, via `UiEvent::KeyDown` to `dioxus-native-dom`.
2. **What data is available?** `physical_key` maps to `Code`, `logical_key` maps to `Key`, `modifiers` are available, and text content maps to `event.text` directly.
3. **Are modifier-only events detectable?** Yes, pressing `Shift` produces a `KeyboardInput` event which is routed normally.
4. **Text content?** Yes, `winit::event::KeyEvent` provides `text: Option<SmolStr>`, which maps to `BlitzKeyEvent::text` so you get `"A"`, `"a"`, or `"€"` without key mapping.
5. **Arrow keys/Nav keys?** Yes, they fire normal `KeyboardInput` events which become `onkeydown` with logical keys like `Key::ArrowUp`.
6. **Shortcuts?** `Ctrl/Cmd + =/-` (Zoom) and `Alt + D/H/T` (Devtools) are intercepted and consumed by `blitz-shell`. Anything else propagates.

## Pointer events

1. **Does `onclick` fire?** Yes, mouse interactions translate to interactions that dioxus-native-dom converts.
2. **`onmousedown`/`onmouseup` separate?** Yes, Dioxus Native's `NativeClickData` receives button state, and blitz forwards pressed/released as `MouseDown` and `MouseUp`.
3. **Coordinate system?** Winit provides Window/Client coordinates (logical pixels). Dioxus exposes this on `ClientPoint`.
4. **Element-local coordinates?** Not automatically provided by Dioxus in blitz. `InteractionElementOffset` returns `unimplemented!()` in `dioxus-native-dom/src/events.rs`.
5. **Continuous `onmousemove`?** Fired based on `CursorMoved` which is fired unconditionally any time the cursor moves across the window regardless of buttons.
6. **Right-click?** Yes, `MouseButton::Right` maps to `MouseEventButton::Secondary` and is exposed.

## Touch events (mobile)

1. **Are touch events exposed?** No. `blitz-shell/src/window.rs` explicitly intercepts `WindowEvent::Touch` and does nothing (`{}`).
2. **Blitz native gesture?** None right now; there is a `Todo implement touch scrolling` comment.
3. **Multi-touch?** Not available. `PinchGesture` and `PanGesture` are also ignored.
4. **Apple Pencil vs Touch?** Impossible to distinguish since none propagate.
5. **Long-press?** Not available organically.

## IME

1. **Does winit expose `WindowEvent::Ime`?** Yes.
2. **Does blitz-shell forward it?** Yes, it converts it to `UiEvent::Ime(winit_ime_to_blitz(..))` and passes it to `self.doc.handle_ui_event`.
3. **Is there a Dioxus handler?** No. `dioxus-native-dom` has `convert_composition_data` hardcoded to `unimplemented!()`.
4. **`set_ime_position` callable?** Not from Dioxus natively; would require accessing the Winit window context.
5. **`set_ime_allowed(true)` required?** It is already hardcoded to `true` in `blitz-shell/src/window.rs` (`winit_window.set_ime_allowed(true)`).
6. **Virtual keyboard on mobile?** Likely won't appear without an explicit trigger if focus isn't completely wired to OS IME events.

## Focus management

1. **Can `<canvas>` receive focus?** Yes, if the element is configured mechanically.
2. **`tabindex="0"` supported?** Yes, Dioxus HTML `tabindex` makes DOM nodes focusable.
3. **`onfocus`/`onblur` fire?** Dioxus will generate these synthetic events internally.
4. **Programmatic focus API?** Dioxus handles programmatic focus via `.focus()` on a mounted ref if blitz supports DOM focus updates.
5. **Keyboard events route to focused elements?** Yes, Blitz routes keyboard input to the focused node first.

## Minimum viable input set

| Event | Available? | Reliable? | Required for editing? | Workaround |
|-------|-----------|-----------|----------------------|------------|
| `onkeydown` & text | Yes | Yes | Yes | None needed |
| `onkeydown` & arrows | Yes | Yes | Yes | None needed |
| `onclick` bounds | Yes | Client only | Yes | Math using global rect |
| `onmousedown` | Yes | Client only | Yes | Math using global rect |
| `onmousemove` drag | Yes | Client only | Yes | Math using global rect |
| IME composition | No | No | Yes (CJK/RTL) | Must defer or patch Dioxus |
| Touch tap | No | No | Yes (mobile) | Must patch Blitz Shell |
| Touch drag | No | No | Yes (mob. sel) | Must patch Blitz Shell |
| `onfocus`/`onblur` | Yes | Yes | Yes | None needed |

## Canvas-specific concerns

1. **Hit Testing:** Canvas hit testing generally uses its bounding rect. This should be identical to a `<div>` as `CustomPaintSource` does not inherently block standard Dioxus HTML pointer checks.
2. **Custom Paint:** Renders completely separately via wgpu commands; it shouldn’t affect Dioxus DOM's own tracking of the element node.
3. **Focus capability:** The canvas will need to be wrapped inside a `<div>` or given `tabindex="0"` natively. It is highly advised to test if Dioxus natively propagates raw text input to a `<canvas>` without wrapping it in a `<input>` offscreen. Often, to support raw IME/mobile, text editors will place a hidden, focused `<textarea>` over the canvas.

## Recommended input strategy for editing

1. **Desktop Keyboard:** Focus the canvas wrapper (or a hidden `<textarea>` sibling overlay) and listen to `onkeydown` directly. `text` property supplies the exact character typed.
2. **Desktop Pointer:** Listen to `onmousedown`, using page coordinates transformed internally against the `WgpuSurface` bounding rect to perform hit testing onto loki layout structures.
3. **Mouse Drag Selection:** Listen to `onmousedown`, store state, listen to window-level `onmousemove` or `onmousemove` on the wrapper, and finalize on `onmouseup`.
4. **Mobile Touch:** **BLOCKED**. Will need to patch `blitz-shell` and `dioxus-native-dom` to forward Winit touch events to Dioxus synthetic events before mobile support can function.
5. **IME:** **BLOCKED**. Will need to patch `dioxus-native-dom` to pass composition data upward, or use a hidden HTML `<textarea>` workaround positioned beneath the cursor to natively capture OS IME and just read Dioxus `oninput`.
6. **Focus:** Create a resilient focus trap by using a wrapper `<div tabindex="0">`. We may additionally embed an invisible `textarea` for native mobile/IME keyboard activation.

## Blockers
- **Touch Event Ignorance:** `blitz-shell` completely ignores `WindowEvent::Touch` and `convert_touch_data` panics with `unimplemented!` in Dioxus Native. Mobile interaction will literally do nothing until this is patched in Blitz/Dioxus.
- **IME Composition Panics:** `dioxus-native-dom`'s `convert_composition_data` panics if called. Advanced text input cannot be cleanly implemented without patching or using hidden native form elements.

## Open questions
- Will a hidden `<input>` or `<textarea>` overlay trigger the OS virtual keyboard natively on mobile within Blitz? Or does Blitz completely ignore web form element keyboards on iOS/Android?
- Do element layout coordinates need to be queried using Dioxus `use_eval` equivilants, or can they be passed down predictably from Taffy through Blitz?
