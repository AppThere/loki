# Macro editor — verification (Phase 7.7)

Verification for the Phase 7 macro editor (spec §3.4). Split into what the test
suite proves automatically and what needs a human at a windowed build.

## Automated (in CI)

The full **import → edit → export → re-import** pipeline is covered end-to-end,
per format, so an edited macro provably survives a real save+reopen through
Loki's own stack:

| What | Test |
|---|---|
| DOCX/VBA: real `.docm` with a real `vbaProject.bin` → edit via write-back → macro-enabled export → re-import reads back the edited source, p-code stripped | `loki-ooxml` `docx::vba_tests::edited_vba_source_survives_docm_round_trip` |
| ODT/Basic: ODT with a Basic module → edit → export → re-import reads back the edited source | `loki-odf` `tests/macro_round_trip.rs::edited_basic_source_survives_export_and_reimport` |
| VBA byte-level write-back (p-code strip, `MODULEOFFSET`=0, minimal `_VBA_PROJECT`, `__SRP_*` removed) | `loki-vba` `write_tests.rs` |
| MS-OVBA compressor round-trips any bytes | `loki-vba` `compress_tests.rs` + `compress_roundtrip` fuzz target |
| ODF module XML rewrite (attrs/decl preserved, entities escaped) | `loki-odf` `basic_write_tests.rs` |
| Save-flow core (`build_edited_payload`, `changed_edits`) | `loki-text` `editor_macro_editor_ops_tests.rs` |
| Trust re-key on edit; external change still drops trust | `loki-macro-host` `service_tests.rs` / `store_tests.rs` |

## Manual (windowed build — not reproducible headless)

The editing **surface** (a Blitz-hosted `<textarea>`) can't be driven in a
headless/GPU-less CI, so these are checked by hand on a desktop build
(`cargo run -p loki-text`). The `<textarea>` capability itself is evidenced at
the source level in [`blitz-textarea-probe.md`](blitz-textarea-probe.md).

Open a macro-enabled document (`.docm` or an ODT with Basic), enable macros
(Document security… → Trust), then **Edit macros…**:

- [ ] The editor panel renders with a module-tab row and a monospace textarea.
- [ ] Typing inserts text; **Enter inserts a newline** (multi-line); Backspace and
      arrow keys work; the caret is visible and tracks edits.
- [ ] Switching module tabs shows that module's source, and switching **back**
      preserves edits made before the switch (drafts survive the remount).
- [ ] **Save** shows the "Macros updated — saving the document" note; the tab's
      unsaved indicator clears after the document write completes.
- [ ] Reopen the saved file **in Loki**: View macros… shows the edited source.
- [ ] Reopen the saved file **in Microsoft Word / LibreOffice**: the macro is
      present and runs — i.e. Office/LibreOffice recompiled it from the
      source-only project (the `_VBA_PROJECT` recompile path, which our own reader
      does not exercise and the round-trip tests therefore cannot cover).

### Known limitation to confirm, not fix

- **IME composition** (CJK etc.) may not register in the textarea:
  `dioxus-native-dom` maps `DomEventData::Ime(_) => None` (upstream `TODO`).
  Latin macro source is unaffected. Note the behaviour; do not treat it as a
  Phase 7 regression.
