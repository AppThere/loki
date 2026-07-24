# Blitz `<textarea>` editability probe (macro editor, Phase 7.5)

**Date:** 2026-07-20
**Question:** Can the macro editor (Phase 7.6) use a real multi-line
`<textarea>` for editing macro source under Dioxus Native / Blitz, or must it
fall back to a bespoke editing surface?

**Verdict: GO — use a real `<textarea>`.** The vendored `blitz-dom` implements a
full multi-line Parley text editor for `<textarea>`, and `dioxus-native-dom`
surfaces its edits as ordinary `oninput` events. The single-line `<input>` the
InputBox already uses (`editor_macro_prompt.rs`) is the same machinery with
`is_multiline = false`.

## Method

This environment is headless (no display/GPU), so a windowed visual run is not
possible here. Instead the probe is **source-level** against the in-tree vendored
patches — which is the authoritative implementation and, unlike a black-box run,
shows the exact mechanism. Citations are `path:line` and verifiable.

## Findings

1. **`<textarea>` builds a multi-line editor.**
   `patches/blitz-dom/src/layout/construct.rs:97` — a `textarea` tag calls
   `create_text_editor(doc, id, /* is_multiline */ true)`; `<input>` takes the
   same path with `false`.

2. **Multi-line keyboard semantics are implemented.**
   `patches/blitz-dom/src/events/keyboard.rs:204` — for a multiline editor,
   `Enter` inserts `"\n"` (a single-line input instead emits `Submit`);
   `Character` inserts text; `Backspace` / word-delete work. It drives a Parley
   editor, the same engine as the main document editor.

3. **Edits reach Dioxus as `oninput`.**
   Keyboard input yields `GeneratedEvent::Input` →
   `DomEventData::Input { value }`
   (`patches/dioxus-native-dom/src/dioxus_document.rs:338`) →
   `NativeFormData { value }` whose `value()` returns the full editor text
   (`patches/dioxus-native-dom/src/events.rs:163`). So `oninput: move |e|
   sig.set(e.value())` — the exact InputBox pattern — yields the whole textarea
   contents, not just the last keystroke.

4. **Initial content is seeded from the `value` attribute, and the editor is
   created once.** `construct.rs:551` — `editor.set_text(element.attr("value")
   .unwrap_or(" "))`, guarded by `if !matches!(special_data, TextInput(_))`. So
   the widget is effectively **uncontrolled**: the initial `value` seeds it, but
   later `value=` changes do **not** re-seed the live editor.

## Integration contract for Phase 7.6

- Render `textarea { value: "{source}", oninput: move |e| draft.set(e.value()), … }`.
  `draft` (a signal) is the source of truth for the save; do **not** try to push
  `draft` back into `value` (finding 4 — it won't take, and would risk desync).
- **Switching module tabs must remount the textarea** so `create_text_editor`
  re-runs with the new module's source: give it `key: "{module_name}"` (or clear
  and rebuild). Without a remount the editor keeps the previous module's text.
- Style it monospace (reuse the viewer's `MONO` stack) and give it a min height;
  document the ≥44×44 logical-px touch target per repo convention.
- An empty module seeds as `" "` (the `unwrap_or(" ")` fallback, finding 4) — the
  save path should treat a lone-space draft as empty source.

## Still to confirm at 7.7 (end-to-end, windowed run)

None of these block the `<textarea>` decision; they are visual / input details to
eyeball when the app is actually run:

- Caret/selection **rendering** and mouse selection (handled by
  `blitz-dom/src/events/mouse.rs`, but not visually confirmed here).
- **IME composition does not currently fire `oninput`** — `dioxus_document.rs`
  maps `DomEventData::Ime(_) => None` with a `// TODO: Implement IME handling`.
  Latin macro source is unaffected; CJK entry via IME may not register. Note this
  as a known limitation rather than a blocker.
