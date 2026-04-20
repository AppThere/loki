# Rendering Fidelity Audit — 2026-04-20

**Branch:** `claude/pagination-reflow-splitting-qJibE`  
**Method:** Code-path analysis (no live rendering); visual comparison against
LibreOffice was not possible because no `.docx` or `.odt` test files exist in
the workspace. This is itself a gap noted at the end of this document.

---

## Test documents

| Document | Available | Notes |
|----------|-----------|-------|
| AppThere Iris Blueprint `.docx` | **No** | Not found anywhere in the workspace |
| Any `.odt` test file | **No** | Not found anywhere in the workspace |

All gaps below were identified by tracing code paths through the import,
model, layout, and render layers, not by visual diff.

---

## Gap inventory

Severity scale: **P0** content missing/unreadable · **P1** clearly wrong at a
glance · **P2** subtle, noticeable on close inspection · **P3** cosmetic only.

Root-cause layers: **A** OOXML/ODF mapper · **B** loki-doc-model missing field ·
**C** loki-layout does not use the field · **D** loki-vello render gap ·
**E** font unavailable · **F** Parley API limitation.

| # | Category | Description | Sev | Layer | File / line |
|---|----------|-------------|-----|-------|-------------|
| 1 | Lists | **DOCX list paragraphs render without markers.** `w:numPr` is mapped to `ParaProps.list_id`/`list_level` but the OOXML mapper always emits `Block::StyledPara`, never `Block::BulletList`/`Block::OrderedList`. The layout engine has no code path to synthesise a marker from `list_id`. ODF lists work correctly via `Block::BulletList`/`Block::OrderedList`. | P0 | C | `loki-layout/src/resolve.rs` (map_para_props drops list_id); `loki-layout/src/flow.rs:224,242` (BulletList/OrderedList paths unreachable from DOCX) |
| 2 | Inline elements | **Footnotes silently discarded.** `Inline::Note` falls through the `_ => {}` arm of `walk_inlines`; all footnote body text is lost. | P0 | C | `loki-layout/src/resolve.rs:219` |
| 3 | Typography | **Superscript / subscript renders at normal baseline.** `CharProps.vertical_align` is fully mapped from `w:vertAlign` (both `Superscript` and `Subscript`) and stored in the model, but `char_props_to_style_span` never copies it to `StyleSpan`, so Parley receives no baseline-shift instruction. | P0 | C | `loki-layout/src/resolve.rs:143–155` (`char_props_to_style_span`); `loki-ooxml/src/docx/mapper/props.rs:196–200` (mapping correct) |
| 4 | Inline elements | **Field codes (page number, date, TOC entries) discarded.** `Inline::Field` produced by the DOCX state machine falls through `walk_inlines` without contributing text or layout items. | P0 | C | `loki-layout/src/resolve.rs:219` |
| 5 | Page layout | **Headers and footers absent.** `DocxSectPr` collects `header_refs` / `footer_refs` relationship IDs but the corresponding XML parts are never loaded or mapped. `LayoutPage.header_items` / `footer_items` are always empty. | P1 | A | `loki-ooxml/src/docx/mapper/mod.rs:142` (stubbed comment); `loki-layout/src/result.rs:88–90` |
| 6 | Paragraph formatting | **Paragraph borders (`w:pBdr`) never rendered.** The OOXML intermediate model `DocxPPr` has no border field; `w:pBdr` is never parsed. ODF likewise lacks border parsing. `ParaProps.border_*` fields and all layout rendering code (`para.rs:327–341`, `loki-vello`) are complete — only the mapper feed is missing. | P1 | A | `loki-ooxml/src/docx/mapper/props.rs` (no border field); `loki-layout/src/para.rs:327–341` (render code exists but never reached) |
| 7 | Paragraph formatting | **Tab stops not rendered.** `w:tabs` is never parsed in the OOXML reader (`DocxPPr` has no tab-stops field). `ParaProps.tab_stops` is always `None`. No `StyleProperty::Tab` call exists in `para.rs`. | P1 | A+C | `loki-ooxml/src/docx/mapper/props.rs` (no tabs field); `loki-layout/src/para.rs` (no Parley tab call) |
| 8 | Paragraph formatting | **Hanging indent not applied.** `indent_hanging` is correctly mapped from `w:ind @w:hanging` (props.rs:108) and stored in `ParaProps`, but `map_para_props` in resolve.rs constructs `ResolvedParaProps` with no `indent_hanging` field. List items and hanging-indent definitions are affected. | P1 | C | `loki-layout/src/resolve.rs:253–303` (`map_para_props`; field absent); `loki-layout/src/para.rs:88` (`ResolvedParaProps` definition) |
| 9 | Images | **Inline images not rendered.** `Inline::Image` (produced by `w:drawing` → data URI mapping) is encountered in `walk_inlines` but only its alt-text string is appended; no `PositionedImage` is ever created. The renderer (`paint_image`) is complete and functional, but the layout layer never feeds it. | P1 | C | `loki-layout/src/resolve.rs:213–216` (`walk_inlines` Image arm); `loki-vello/src/image.rs:29–68` (renderer complete) |
| 10 | Typography | **Highlight colour ignored.** `CharProps.highlight_color` is fully parsed from `w:highlight` (16 named colours) and stored in the model, but `char_props_to_style_span` does not include it in `StyleSpan`. No background-colour pass is generated for highlighted runs. | P1 | C | `loki-layout/src/resolve.rs:143–155` |
| 11 | Inline elements | **Hyperlink URLs lost.** `walk_inlines` extracts the text content of `Inline::Link` but discards the URL. Links render as unstyled plain text. | P1 | C | `loki-layout/src/resolve.rs:213–216` |
| 12 | Images | **External URL images render as grey placeholder.** `paint_image` only decodes `data:` URIs; any `http://`, `https://`, or `file://` `src` receives a `FilledRect` grey box. Intentional for v0.1 but still a visible gap. | P1 | D | `loki-vello/src/image.rs:34–42` |
| 13 | Typography | **Letter spacing not applied.** `CharProps.letter_spacing` is mapped from `w:spacing` (twips → points, props.rs:189) but never reaches `StyleSpan` or Parley. | P2 | C | `loki-layout/src/resolve.rs:143–155` |
| 14 | Typography | **Horizontal text scale not applied.** `CharProps.scale` is mapped from `w:w` (percentage, props.rs:192) but never reaches `StyleSpan` or Parley. | P2 | C | `loki-layout/src/resolve.rs:143–155` |
| 15 | Typography | **Small caps not applied.** `CharProps.small_caps` is parsed (props.rs:150) but absent from `StyleSpan`. | P2 | C | `loki-layout/src/resolve.rs:143–155` |
| 16 | Typography | **All caps not applied.** `CharProps.all_caps` is parsed (props.rs:151) but absent from `StyleSpan`. | P2 | C | `loki-layout/src/resolve.rs:143–155` |
| 17 | Typography | **Underline style variety collapsed to single.** `UnderlineStyle` enum (Single / Double / Dotted / Dash / Wave / Thick) is fully parsed and stored, but `StyleSpan.underline` is a `bool`. All underline variants render identically. | P2 | C | `loki-layout/src/para.rs:50` (`StyleSpan`); `loki-layout/src/resolve.rs:152` (mapping) |
| 18 | Typography | **Double strikethrough collapsed.** `StrikethroughStyle::Double` (from `w:dstrike`) is correctly mapped but `StyleSpan.strikethrough` is a `bool`; double and single strikethrough are indistinguishable. | P2 | C | `loki-layout/src/para.rs:50`; `loki-layout/src/resolve.rs:153` |
| 19 | Paragraph formatting | **RTL / bidi not applied.** `ParaProps.bidi` is parsed from `w:bidi` (props.rs:122) but `map_para_props` does not include it in `ResolvedParaProps`. | P2 | C | `loki-layout/src/resolve.rs:253–303` |
| 20 | Paragraph formatting | **`page_break_after` not implemented.** `ParaProps.page_break_after` exists in the model (para_props.rs:181) but is absent from `ResolvedParaProps` and never acted on. | P2 | C | `loki-layout/src/resolve.rs:253–303` |
| 21 | Typography | **`lineRule="atLeast"` not enforced.** `ResolvedLineHeight::AtLeast(pts)` is resolved correctly but the matching arm in `para.rs:208` is a no-op comment: "good enough for v0.1". Lines shorter than the minimum will be too tight. | P2 | F | `loki-layout/src/para.rs:204–211`; Parley 0.6 has no native AtLeast API |
| 22 | Typography | **Word spacing not applied.** `CharProps.word_spacing` parsed but not in `StyleSpan`. | P3 | C | `loki-layout/src/resolve.rs:143–155` |
| 23 | Typography | **Kerning flag ignored.** `CharProps.kerning` boolean parsed (props.rs:186) but not in `StyleSpan`; Parley applies its own default kerning regardless. | P3 | C | `loki-layout/src/resolve.rs:143–155` |
| 24 | Typography | **Shadow text not applied.** `CharProps.shadow` parsed (props.rs:152) but not in `StyleSpan`. | P3 | C | `loki-layout/src/resolve.rs:143–155` |
| 25 | Paragraph formatting | **Orphan / widow control not implemented.** `widow_control` / `orphan_control` are in `ParaProps` and mapped but absent from `ResolvedParaProps`. | P3 | C | `loki-layout/src/resolve.rs:253–303` |
| 26 | Paragraph formatting | **`border_between` ignored.** `ParaProps.border_between` (the rule between adjacent same-styled paragraphs) is never included in `map_para_props`. | P3 | C | `loki-layout/src/resolve.rs:253–303` |
| 27 | Document settings | **`DocxSettings` silently skipped.** Default tab stop, `evenAndOddHeaders`, and `titlePg` flags are parsed but have no `loki-doc-model` target. | P3 | B | `loki-ooxml/src/docx/mapper/mod.rs:183` (TODO comment) |
| 28 | Tables | **Table cell vertical merge stubbed.** `DocxTcPr.v_merge` is parsed but `row_span` is always emitted as 1. | P3 | A | `loki-ooxml/src/docx/mapper/table.rs` |
| 29 | Document structure | **Content controls (`w:sdt`) silently skipped.** `DocxBodyChild::Sdt` has no model equivalent and is dropped. | P3 | A | `loki-ooxml/src/docx/mapper/mod.rs:145` |
| 30 | Typography | **Language tags ignored.** `CharProps.language` parsed but not in `StyleSpan`; no hyphenation or locale-sensitive shaping. | P3 | C | `loki-layout/src/resolve.rs:143–155` |

---

## Known TODO items

| File | Line | Comment text | Related gap # |
|------|------|--------------|---------------|
| `loki-ooxml/src/docx/mapper/mod.rs` | 183 | `TODO(mapper): DocxSettings has no loki-doc-model equivalent yet — skipped.` | #27 |
| `loki-layout/src/para.rs` | 152 | `TODO(split-optimise): Option B y-range item filter can use this field` | — (performance only) |
| `loki-text/src/routes/editor.rs` | 270 | `TODO(odt-fidelity): DOCX rendering gaps (styles, page size) tracked separately.` | General DOCX fidelity |
| `loki-text/src/routes/editor.rs` | 275 | `TODO(odt-fidelity): ODT rendering gaps — some paragraph styles, list indents, and image placement may not render correctly yet.` | #8, #9 |
| `loki-text/src/components/document_source.rs` | 325 | `TODO(partial-render): pass visible_rect as clip region to paint_layout` | — (performance only) |
| `loki-text/src/components/wgpu_surface.rs` | 42 | `TODO(partial-render): wire scroll_offset → visible_rect → LokiDocumentSource` | — (performance only) |
| `loki-text/src/routes/editor.rs` | 118 | `TODO(partial-render): wire scroll_offset → visible_rect once Blitz …` | — (performance only) |
| `loki-vello/src/scene.rs` | 24 | `TODO(shadow): replace with Vello blur filter once rendering is verified stable.` | — (page shadow rendering, not content fidelity) |
| `loki-vello/src/scene.rs` | 53 | `TODO(partial-render): page_index is the first step toward viewport clipping` | — (performance only) |

---

## Recommended fix order

### Group 1 — Expand `StyleSpan` (Layer C, multiple P0/P1/P2 unblocked)

A single change to `StyleSpan` and `char_props_to_style_span` in
`loki-layout/src/resolve.rs` + corresponding Parley calls in `para.rs` unblocks
the largest cluster of gaps. Each sub-item is independently shippable.

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Add `vertical_align` field → Parley font-size × 0.58 shift + smaller font-size for super/sub | #3 (P0) | S |
| Add `highlight_color` field → emit a background `FilledRect` behind glyph runs | #10 (P1) | S |
| Add `letter_spacing` field → `StyleProperty::LetterSpacing` per span | #13 (P2) | S |
| Add `small_caps` / `all_caps` fields → `StyleProperty::FontVariantCaps` | #15, #16 (P2) | S |
| Replace `underline: bool` with `underline_style: Option<UnderlineStyle>` | #17 (P2) | S |
| Replace `strikethrough: bool` with `strikethrough_style: Option<StrikethroughStyle>` | #18 (P2) | S |
| Add `word_spacing` → `StyleProperty::WordSpacing` | #22 (P3) | S |
| Add `shadow` → synthesised blur rect behind glyph run (or defer to Vello blur) | #24 (P3) | S |

### Group 2 — Expand `ResolvedParaProps` (Layer C, targeted fixes)

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Add `indent_hanging` → applied as `layout_width` adjustment and first-line offset in `para.rs` | #8 (P1) | S |
| Add `page_break_after` → handled in `place_paragraph_layout` after cursor advance | #20 (P2) | S |
| Add `bidi` → `builder.set_bidi_level` if Parley exposes it | #19 (P2) | M |
| Add `orphan_control` / `widow_control` → post-split minimum line count in `split_and_place_loop` | #25 (P3) | M |

### Group 3 — DOCX list marker synthesis (Layer C, P0)

The OOXML mapper correctly stores `list_id` + `list_level` in `ParaProps`
and the `StyleCatalog` holds fully resolved `ListStyle` entries. The missing
piece is a step in `flow_para.rs` (before `flow_paragraph`) that detects
`StyledPara` with a `list_id`, looks up the `ListStyle`, resolves the level
counter, and injects the marker text (bullet char or formatted number) as a
`Inline::Str` prefix—matching the ODF path. A running counter per `list_id`
must be maintained in `FlowState`. This is the highest-impact single fix.

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Inject list markers in `flow_para.rs` from `list_id`/`list_level` | #1 (P0) | M |

### Group 4 — Add `Inline::Image` to layout (Layer C, P1)

`walk_inlines` currently drops `Inline::Image`; `PositionedImage` is never
produced by `flatten_paragraph`. Fix requires `para.rs` to intercept
`Inline::Image` before handing control to Parley (or a pre-pass that
extracts images and their approximate inline positions), then emit
`PositionedItem::Image` at the correct `(x, y)` offset after line breaking.

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Handle `Inline::Image` in layout flattening / para.rs | #9 (P1) | M |

### Group 5 — Layer A: parse missing OOXML properties

Both fixes require a new field in the intermediate model (`DocxPPr`) and
reader XML parsing before the mapper can propagate them.

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Parse `w:pBdr` → `DocxPBdr` in intermediate model → map to `ParaProps.border_*` | #6 (P1) | M |
| Parse `w:tabs` → `DocxTabs` in intermediate model → map to `ParaProps.tab_stops` → Parley tab stop calls | #7 (P1) | M |

### Group 6 — Footnotes and field codes (Layer C, P0)

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Render field codes: at minimum emit snapshot text (from `Field.snapshot`) for common kinds (PAGE, DATE) | #4 (P0) | S–M |
| Footnotes: requires a placement policy decision (end-of-page, end-of-section). Render body at minimum as end-of-section blocks. | #2 (P0) | L |

### Group 7 — Headers and footers (Layer A, P1)

Requires parsing the header/footer XML parts (relationship resolution in the
OOXML reader), a new `loki-doc-model` representation, and a separate layout
pass to populate `LayoutPage.header_items` / `footer_items`.

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Parse header/footer XML → new model type → layout pass | #5 (P1) | L |

### Group 8 — AtLeast line height (Layer F, P2)

Parley 0.6 has no `LineHeight::AtLeast` variant. The workaround is to
measure the natural height of the paragraph and, if it falls below the
threshold, re-run `break_all_lines` with `LineHeight::Absolute(pts)`.

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Post-measure height check → re-layout with exact if below minimum | #21 (P2) | M |

### Group 9 — P3 cosmetic

| Fix | Unblocks gap(s) | Scope |
|-----|-----------------|-------|
| Add `border_between` to `map_para_props` and emit between adjacent same-styled paragraphs | #26 (P3) | M |
| `DocxSettings` → new `DocumentSettings` model type | #27 (P3) | M |
| Table `v_merge` → `row_span` in cell spanning logic | #28 (P3) | M |
| Language tags → Parley locale (once Parley exposes per-span locale) | #30 (P3) | L |

---

## Estimated scope summary

| Scope | Groups | Gap count unblocked |
|-------|--------|---------------------|
| S (single session) | Group 1 (most items), Group 6 (field codes) | ~10 gaps |
| M (multi-session) | Groups 2–5, 7 partial, 8, 9 | ~14 gaps |
| L (large / deferred) | Group 6 (footnotes), Group 7 (headers/footers), Group 9 (language) | ~4 gaps |

---

## Overall fidelity state

The pipeline is architecturally sound: the OOXML and ODF mappers faithfully
parse and store the vast majority of DOCX/ODT properties into a rich
`loki-doc-model`, and the Vello renderer can handle every `PositionedItem`
variant it is given. The fidelity shortfall is concentrated almost entirely in
the middle layer — `loki-layout`'s `resolve.rs` and `para.rs` — where roughly
fifteen `CharProps` and `ParaProps` fields that are correctly parsed and stored
are simply not forwarded to Parley or reflected in layout decisions. The four
highest-severity (P0) gaps are all in this layer and are independently
fixable in single sessions; the most impactful single fix is DOCX list marker
synthesis (gap #1), which would make the most visually prominent class of
documents render correctly. The remaining P1 gaps — headers/footers and
paragraph borders — require new OOXML reader parsing but the downstream model
and render infrastructure is already in place. The absence of any test `.docx`
or `.odt` document in the workspace is itself a gap that should be addressed
before any fidelity fix ships: without a reference document, regressions are
invisible.
