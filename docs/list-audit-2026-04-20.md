# List Data Pipeline Audit — 2026-04-20

**Scope:** Fidelity gap #1 — DOCX list paragraphs render without markers.
**Conclusion:** All data is present in `StyleCatalog` and `ParaProps`; the
entire gap is in `loki-layout` (property resolution + counter tracking +
marker synthesis). One implementation session needed.

---

## Gap table

| Item | Available? | Location |
|------|:----------:|----------|
| `numbering.xml` parsed | ✅ | `loki-ooxml/src/docx/reader/numbering.rs:21–58` |
| `<w:abstractNum>` levels parsed | ✅ | `loki-ooxml/src/docx/model/numbering.rs:71–78` |
| `<w:num>` instances parsed | ✅ | `loki-ooxml/src/docx/model/numbering.rs:80–89` |
| `numId → abstractNumId` indirection resolved | ✅ | `loki-ooxml/src/docx/mapper/numbering.rs:169–193` |
| Level overrides applied | ✅ | `loki-ooxml/src/docx/model/numbering.rs:56–68` (`ResolvedNumDef::level`) |
| Per-level: bullet char / number format | ✅ | `loki-ooxml/src/docx/mapper/numbering.rs:91–137` → `ListLevelKind` |
| Per-level: indent / hanging indent | ✅ | `loki-ooxml/src/docx/mapper/numbering.rs:52–66` (twips → pts) |
| Per-level: start value | ✅ | `loki-ooxml/src/docx/mapper/numbering.rs:76` |
| `StyleCatalog` holds `ListStyle` keyed by `ListId` | ✅ | `loki-doc-model/src/style/catalog.rs:83` (`IndexMap<ListId, ListStyle>`) |
| `ParaProps.list_id` / `list_level` set | ✅ | `loki-ooxml/src/docx/mapper/props.rs:131–136` |
| `ResolvedParaProps` carries list fields | ❌ | Dropped in `loki-layout/src/resolve.rs` |
| `FlowState` list counter | ❌ | No list fields in `loki-layout/src/flow.rs:99–124` |
| Marker synthesis for DOCX paragraphs | ❌ | Never reached; `StyledPara` path has no marker logic |

---

## Pipeline summary

```
numbering.xml
  → DocxAbstractNum / DocxNum          (reader/numbering.rs)
  → map_numbering() resolves overrides  (mapper/numbering.rs:149–206)
  → ListStyle { levels: Vec<ListLevel> } inserted into StyleCatalog

paragraph <w:numPr>
  → DocxNumPr { ilvl, num_id }          (model/paragraph.rs:91–98)
  → map_ppr() sets ParaProps.list_id / list_level  (mapper/props.rs:131–136)
  → Block::StyledPara with direct_para_props containing list_id + list_level

loki-layout: resolve_para_props()
  → reads ParaProps → ResolvedParaProps
  → list_id / list_level SILENTLY DROPPED  ← entire gap is here
```

---

## Minimum work to synthesise a correct marker string

1. **Extend `ResolvedParaProps`** with:
   - `list_id: Option<ListId>`
   - `list_level: Option<u8>`

2. **Forward in `resolve_para_props()`** (resolve.rs) — copy both fields
   from `ParaProps` into `ResolvedParaProps`.

3. **Extend `FlowState`** with a per-list, per-level counter map:
   `list_counters: HashMap<ListId, [u32; 9]>`

4. **In `flow_block()` for `Block::StyledPara`**, when `resolved.list_id.is_some()`:
   a. Look up `catalog.list_styles.get(&list_id)` → `ListStyle`
   b. Get `ListLevel` at `list_level` index
   c. Advance (or reset) the counter for this list+level
   d. Format the marker string from `ListLevelKind`:
      - `Bullet { char, .. }` → `"{char} "`
      - `Numbered { scheme, format, start_value, display_levels, .. }` →
        render format string replacing `%N` tokens with counter values
      - `None` → no marker
   e. Prepend the marker as a short `Inline::Str` (same as the ODF path does)
   f. Apply `hanging_indent` from `ListLevel` as a negative first-line indent

5. **Counter reset rule**: when the level decreases back to a lower level,
   reset all deeper-level counters for that list ID. When a new list starts
   (unseen `list_id`), initialise fresh counters at `start_value` for all
   levels.

Total: ~120 lines across `para.rs`, `resolve.rs`, `flow.rs`.

---

## ODF list path — reuse potential

`flow.rs:224–260` (`Block::BulletList` / `Block::OrderedList`) does crude
hardcoded injection of `"• "` / `"N. "` strings with a fixed 18 pt indent.
It is **not reusable** for DOCX — the ODF path ignores `ListLevel` indent and
does not respect `hanging_indent`, `label_alignment`, or `char_props` on the
label. The marker-synthesis logic for DOCX should be written fresh alongside
the `StyledPara` path, and the ODF path should eventually be refactored to
call the same routine (separate task).

---

## OOXML-specific complexities to handle

| Complexity | Detail | Risk |
|-----------|--------|------|
| `num_id == 0` | Explicit "remove numbering" sentinel — already guarded in mapper (`props.rs:98`) | None |
| Multi-level `display_levels` | Numbered format `"%2.%3."` shows ancestor counters; `display_levels` field counts `%N` tokens | Medium — format string walk needed |
| `<w:lvlOverride>` with `startOverride` | Per-instance restart of a specific level counter | Medium — already parsed in `DocxLvlOverride`; need restart logic in FlowState |
| Custom bullet fonts | `ListLevel.char_props` may carry a symbol font (e.g. Wingdings) | Low for now — ignore font, use char as-is |
| Picture bullets | `BulletChar::Image` | Low — render as `'•'` fallback |
| List continuation vs. restart | Two paragraphs with same `list_id` at level 0 may or may not restart; determined by whether `start_value` override is present | Low — no restart unless `lvlOverride.startOverride` is set |
