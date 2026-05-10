# ADR 007: Document Export Pipeline

## Status

Proposed

## Date

2026-05-10

## Context

Both `loki-ooxml` and `loki-odf` are import-only. Their export entry points
exist as correctly-typed stubs that unconditionally return an error:

- `loki-ooxml/src/docx/export.rs` — `DocxExport::export` returns
  `OoxmlError::ExportNotImplemented`.
- `loki-odf/src/odt/export.rs` — `OdtExport::export` returns
  `OdfError::NotImplemented { feature: "ODT export" }`.

Both implement the `loki_doc_model::io::DocumentExport` trait, which accepts a
`&Document` and a `impl Write + Seek`. The trait surface and error types are
already in place; only the implementation is missing.

### LoroDoc → Document bridge completeness

`loki-doc-model` contains a `loro_bridge` module that converts between
`LoroDoc` and `Document` in both directions (ADR-0006). The bridge is **partially
implemented**:

| Block variant | `document_to_loro` (write) | `loro_to_document` (read) |
|--------------|--------------------------|--------------------------|
| `StyledPara` / `Para` | ✅ implemented | ✅ implemented |
| `Heading` | ✅ implemented | ✅ implemented |
| `BulletList` / `OrderedList` | ⚠️ stubbed (debug log only) | ⚠️ stubbed (debug log only) |
| `Table` | ⚠️ stubbed (debug log only) | ⚠️ stubbed (debug log only) |
| `Figure` / `BlockQuote` / `Div` | ⚠️ stubbed (debug log only) | ⚠️ stubbed (debug log only) |

For v0.1 export (Option A — serialize from `Document` snapshot, see Decision 4
below), the Loro bridge completeness does not block export: the exporter reads
from a `loki-doc-model::Document` directly, not from a `LoroDoc`. Lists,
tables, and figures are fully represented in the `Document` type even where
the Loro bridge stubs them.

---

## Research findings

### OOXML write

#### Minimum OPC parts for a valid `.docx`

A minimal OOXML document requires six parts. Additional parts are needed for
styles, numbering, headers/footers, and images.

| Part | Required? | Notes |
|------|-----------|-------|
| `[Content_Types].xml` | ✅ mandatory | OPC §10 |
| `_rels/.rels` | ✅ mandatory | OPC §9; must have officeDocument rel |
| `word/document.xml` | ✅ mandatory | ECMA-376 §17.2 |
| `word/_rels/document.xml.rels` | ✅ mandatory | references styles, settings, etc. |
| `word/styles.xml` | ✅ mandatory | ECMA-376 §17.7; must exist even if empty |
| `word/settings.xml` | ✅ mandatory | ECMA-376 §17.15; required for `w:defaultTabStop` etc. |
| `word/numbering.xml` | ⚠️ if lists present | ECMA-376 §17.9 |
| `word/header*.xml` / `word/footer*.xml` | ⚠️ if headers/footers present | ECMA-376 §17.10 |
| `word/footnotes.xml` / `word/endnotes.xml` | ⚠️ if notes present | ECMA-376 §17.11 |
| `word/media/image*` | ⚠️ if images present | embedded image blobs |

`loki-opc` already has full write support (`Package::write`,
`write_package_to_zip`), producing correct `[Content_Types].xml` and all
`_rels` part files automatically from the `Package` in-memory model. The OOXML
export path will populate an `OpcPackage` with raw XML byte strings per part
and call `Package::write`.

#### Existing Rust OOXML write crates

| Crate | Version | License | Model | Assessment |
|-------|---------|---------|-------|------------|
| `docx` | 1.1.2 | MIT | Own AST | Requires constructing its own document model; MIT ✓ but the AST is unrelated to `loki-doc-model`. Bridging would require writing a full second mapper. Not worth it. |
| `docx-rs` | 0.4.20 | MIT | Own AST | WASM-first; active but similarly has its own model. No benefit over hand-crafted XML. |
| `ooxml` | 0.2.8 | Unknown | Schema-generated | Alpha; uses generated types from RELAX NG schema; not suitable. |

**Conclusion:** No existing crate reduces implementation effort sufficiently.
The OOXML import code already understands the XML structure; reversing it using
`quick-xml` for serialization is the right approach. External crates would
add a dependency while requiring a second mapping layer with no net saving.

### ODF write

#### Minimum parts for a valid `.odt`

| Part | Required? | Notes |
|------|-----------|-------|
| `mimetype` | ✅ mandatory | Must be first ZIP entry, stored/uncompressed. ODF §3.4. |
| `META-INF/manifest.xml` | ✅ mandatory | Lists all parts. ODF §3.9. |
| `content.xml` | ✅ mandatory | Document body. ODF §3.7. |
| `styles.xml` | ✅ mandatory | Named and automatic styles. ODF §3.7. |
| `meta.xml` | ⚠️ recommended | Document metadata (title, author, dates). ODF §3.7. |
| `settings.xml` | ⚠️ recommended | Application settings. ODF §3.7. |
| `Pictures/*` | ⚠️ if images present | Embedded image blobs. |

ADR-0001 records that `loki-odf` uses the `zip` crate directly (not `loki-opc`)
for reading, because ODF's `META-INF/manifest.xml` structure has no equivalent
in OPC. The same approach applies to writing: construct the ZIP directly using
the `zip` crate (already a dependency of `loki-odf`), writing entries in ODF
§3.4 order (`mimetype` first, stored).

ADR-0002 establishes that the detected `office:version` from `document.source`
must be written back unchanged.

#### Existing Rust ODF write crates

| Crate | Version | License | Assessment |
|-------|---------|---------|------------|
| `lo_odf` | 0.4.7 | MIT | Targets ODF serialization for the `clark-labs-inc/libreoffice-rs` project. API is tailored to their model, not `loki-doc-model`. Would require the same bridging cost as the OOXML crates. Not suitable. |

**Conclusion:** Same as OOXML — hand-crafting the XML with `quick-xml` is
preferable. The ODF import reader is the structural inverse; the export writer
can mirror it.

---

## Decisions

### Decision 1: Crate structure — Option A (export in existing import crates)

**Rejected: Option B — separate export crates.**
`loki-ooxml` and `loki-odf` already contain the complete intermediate models,
XML reader infrastructure, and format-specific knowledge. Splitting them into
`loki-ooxml` + `loki-ooxml-write` would require either duplicating the
intermediate model or re-exporting it through a shared crate. The workspace
already has 11 crates; adding two more for stubs-turned-implementations adds
overhead with no concrete benefit at this scale.

**Rejected: Option C — unified `loki-export` crate.**
OOXML uses `loki-opc` for its OPC packaging; ODF uses plain `zip`. Their XML
namespaces, element structures, and package conventions are entirely different.
A unified crate would share nothing except the `DocumentExport` trait, which
already lives in `loki-doc-model`.

**Chosen: Option A.**

Add a `write` submodule to `loki-ooxml/src/docx/` and `loki-odf/src/odt/`,
implementing the existing stub functions. The intermediate model types
(`DocxParagraph`, `DocxRun`, `OdfStyle`, etc.) are co-located and accessible
without any re-export.

### Decision 2: Intermediate model reuse — Option B (direct serialization from doc-model)

**Rejected: Option A — reuse import intermediate model for export.**
The import intermediate model is designed for reading: it captures
format-specific parsing artefacts (e.g. `DocxBorderEdge`, `OdfParaProps`) that
exist as a staging layer between raw XML and the clean `loki-doc-model` types.
Constructing these intermediate types from `loki-doc-model` during export
would invert the mapping unnecessarily and create bidirectional coupling that
makes each layer harder to evolve independently. For example, `DocxParagraph`
contains `DocxPPr` which contains `DocxPBdr` — these types exist to parse XML,
not to generate it.

**Chosen: Option B — direct serialization from `loki-doc-model` types to XML.**

The export module walks `loki-doc-model::Document` and emits XML directly using
`quick-xml`'s `Writer` API (already in the dependency trees of both crates as a
transitive dependency of the readers). This produces clean, minimal output
without recreating import-specific intermediate structures.

Implication: some format-specific knowledge currently embedded only in the
reader (e.g. the mapping between `ParagraphAlignment::Justify` and `w:jc
val="both"` in OOXML) must be extracted into a shared `mapper/` location or
duplicated in the writer. For v0.1 the duplication is acceptable; extraction
is an optimization for a later pass.

### Decision 3: Round-trip fidelity — Tier 3 (content-only round-trip) for v0.1

**Tier 1 (lossless)** — not achievable in v0.1 because the import pipeline
intentionally drops comments, tracked changes, content controls, and
format-specific extension data (stored in `ExtensionBag` but not mapped back
to XML). Making export lossless would require threading all dropped data
through the model, which is a larger change than the export implementation
itself.

**Tier 2 (faithful)** — achievable in principle but requires exporting headers,
footers, footnotes, endnotes, and list numbering definitions, all of which
involve non-trivial XML structure. This is the right eventual target but not
a v0.1 constraint.

**Chosen: Tier 3 (content-only) for v0.1.** The first export must produce a
file that opens correctly in LibreOffice/Word and preserves all rendered
content. Concretely:

- All block types in the v0.1 scope list below are serialized.
- Named style references (`style_id`) and the style catalog are exported.
- Page size and margins (from `Section.layout`) are exported in `w:sectPr` /
  `style:page-layout`.
- Document settings (tab stop interval) are exported in `word/settings.xml` /
  `settings.xml`.
- Headers and footers are exported (the `Section.layout` fields already carry
  them as `loki-doc-model` block trees).
- Footnotes and endnotes are exported (they appear as `Inline::Note` and are
  already fully represented in `loki-doc-model`).
- Comments, tracked changes, content controls, bookmarks, cross-references,
  and `ExtensionBag` data are silently dropped.

### Decision 4: Export source — Option A (serialize from `loki-doc-model::Document`)

**Rejected: Option B — serialize directly from `LoroDoc`.**
Would require a second, parallel serialization path entirely independent of the
import pipeline. The Loro schema (ADR-0006) maps document content into Loro
containers using a different structure than OOXML/ODF XML. Writing a
`LoroDoc → DOCX` serializer is strictly more work than `Document → DOCX` and
provides no additional fidelity at v0.1 (since the Loro bridge has the same
completeness as the `Document` model).

**Rejected: Option C — hybrid pass-through.**
Requires tracking edit provenance at the XML level, which is a
multi-milestone project on its own.

**Chosen: Option A.** Call `loro_to_document()` (when saving from an editing
session) or use the `Document` directly (when re-exporting an imported file).
Pass the resulting `Document` to `DocxExport::export` / `OdtExport::export`.

This is consistent with the architecture established in ADR-0006: `Document` is
the derived read-only snapshot; it is the stable interface between format
crates and the rest of the system. The export crates know nothing about Loro.

### Decision 5: OPC/ZIP write infrastructure — Option A (extend loki-opc, Option B for ODF)

**For OOXML:** `loki-opc` already has full write support:
`Package::write(&self, impl Write + Seek)` calls `write_package_to_zip`, which
correctly generates `[Content_Types].xml` and all `_rels/` parts.
`loki-ooxml` already depends on `loki-opc`. The OOXML export path will
construct an `OpcPackage`, add parts as `PartData` byte vectors, and call
`Package::write`. No new infrastructure needed.

**For ODF:** ADR-0001 established that `loki-odf` does not use `loki-opc`.
The `zip` crate is already a direct dependency of `loki-odf`. The ODF export
path will use `ZipWriter` directly (same as the test helpers in
`loki-odf/tests/helpers.rs`), writing `mimetype` first as a stored entry,
then the remaining XML parts as deflated entries, then `META-INF/manifest.xml`
last (or second — ODF §3.4 only mandates `mimetype` first).

---

## Implementation order

Each step should be followed by `cargo check --workspace` and the relevant
integration tests before proceeding.

1. **OOXML body serializer** — `loki-ooxml/src/docx/write/body.rs`.
   Walk `Document.sections[*].blocks` and emit `<w:body>` XML for the v0.1
   scope: `StyledPara`, `Para`, `Heading`, `BulletList`, `OrderedList`, `Table`.
   Use `quick-xml::Writer`. No packaging yet — return raw bytes.

2. **OOXML styles serializer** — `loki-ooxml/src/docx/write/styles.rs`.
   Emit `word/styles.xml` from `Document.styles` (paragraph and character
   styles). Re-use the `CharProps`/`ParaProps` → XML attribute mapping,
   which is the inverse of `map_text_props`/`map_para_props`.

3. **OOXML package assembly** — `loki-ooxml/src/docx/write/package.rs`.
   Assemble the `OpcPackage`: add content-typed parts for `document.xml`,
   `styles.xml`, `settings.xml`, `numbering.xml` (if lists present),
   `footnotes.xml` (if notes present), and header/footer parts. Wire
   relationships. Call `Package::write`. Implement `DocxExport::export`.

4. **OOXML round-trip integration test** — open the reference DOCX from
   `loki-ooxml/tests/helpers.rs`, export it, re-import the result, and assert
   that block count, style names, and key formatting properties are preserved.

5. **ODF body serializer** — `loki-odf/src/odt/write/body.rs`.
   Emit `<office:body><office:text>` from the same block set. ODF uses
   `text:p`, `text:h`, `text:list`, `table:table` — different elements but
   same structural logic.

6. **ODF styles serializer** — `loki-odf/src/odt/write/styles.rs`.
   Emit `content.xml` auto-styles and `styles.xml` named styles. Preserve
   `document.source.version` per ADR-0002.

7. **ODF package assembly** — `loki-odf/src/odt/write/package.rs`.
   Construct the ZIP directly with `ZipWriter`: `mimetype` (stored), then
   `content.xml`, `styles.xml`, `meta.xml`, `settings.xml`,
   `META-INF/manifest.xml`. Implement `OdtExport::export`.

8. **ODF round-trip integration test** — mirror the OOXML step for ODT.

---

## Minimum viable export (v0.1 scope)

The following must work for a v0.1 export to be considered shippable. "Works"
means: the exported file opens in LibreOffice Writer and Microsoft Word without
an error dialog, and the listed content appears correctly.

**Content (Block variants):**

| Block variant | OOXML element | ODF element |
|--------------|--------------|------------|
| `StyledPara` | `<w:p>` with `<w:pStyle>`, `<w:pPr>`, `<w:rPr>` | `<text:p text:style-name>` |
| `Para` | `<w:p>` (no style) | `<text:p>` |
| `Heading(1..6)` | `<w:p>` with `Heading1`–`Heading6` style | `<text:h text:outline-level>` |
| `BulletList` | `<w:p>` with `<w:numPr>` + `word/numbering.xml` | `<text:list>` |
| `OrderedList` | `<w:p>` with `<w:numPr>` + `word/numbering.xml` | `<text:list>` |
| `Table` | `<w:tbl>` with `<w:tr>`, `<w:tc>` | `<table:table>` etc. |
| `HorizontalRule` | `<w:p>` with `<w:pBdr>` bottom single | `<text:p>` with `fo:border-bottom` |

**Inline variants:**

| Inline variant | OOXML | ODF |
|----------------|-------|-----|
| `Str` | `<w:t>` | text content |
| `StyledRun` | `<w:r>` with `<w:rPr>` | `<text:span>` |
| `Space` | `<w:t xml:space="preserve"> </w:t>` | `<text:s/>` |
| `LineBreak` | `<w:br/>` | `<text:line-break/>` |
| `Strong` / `Emph` | `<w:r>` with `<w:b>` / `<w:i>` | `<text:span>` with `fo:font-weight` |
| `Link` | `<w:hyperlink>` | `<text:a>` |
| `Note(Footnote)` | `<w:footnoteReference>` | `<text:note text:note-class="footnote">` |
| `Field` | `<w:fldChar>` / `<w:instrText>` sequence | typed element (`<text:page-number>` etc.) |

**Formatting properties:**
- All `CharProps` fields that have corresponding OOXML/ODF elements in the
  existing import mapper (bold, italic, underline, strikethrough, color,
  highlight, font name, font size, vertical align, letter spacing,
  small-caps, all-caps, shadow).
- All `ParaProps` fields: alignment, indent (start/end/first-line/hanging),
  space before/after, line height, keep-together, keep-with-next,
  page-break-before, tab stops, border (all four sides), background color.

**Document structure:**
- Page size and margins from `Section.layout.page_size` and
  `Section.layout.margins` (OOXML: `<w:pgSz>`, `<w:pgMar>`; ODF:
  `style:page-layout-properties`).
- Default tab stop from `Document.settings.default_tab_stop_pt` (OOXML:
  `<w:defaultTabStop>`; ODF: not applicable — ODF uses per-paragraph tab stops).
- Style catalog: all paragraph styles and character styles from
  `Document.styles`.
- Headers and footers: `Section.layout.header`, `footer`, `header_first`,
  `footer_first`, `header_even`, `footer_even`.
- Footnotes and endnotes: all `Inline::Note` content in the document.

---

## Out of scope for v0.1

- Comments (`w:comment`, ODF `office:annotation`)
- Tracked changes / revision marks (`w:ins`, `w:del`, ODF `text:change*`)
- Content controls (`w:sdt`)
- Bookmarks and cross-references (beyond simple hyperlinks)
- Math content (`w:oMath`, ODF `draw:object` MathML)
- Embedded charts or spreadsheets
- Custom XML parts
- Document protection / encryption
- `ExtensionBag` pass-through (format-specific data not mapped to
  `loki-doc-model` is silently dropped)
- `TableOfContents`, `Index` block variants (require field codes)
- `Figure` block variant (requires image embedding; deferred to v0.2)
- `DefinitionList`, `LineBlock`, `CodeBlock`, `BlockQuote`, `Div` block
  variants (deferred; uncommon in office documents)
- OOXML-specific `w:next` style chaining beyond what `next_style_id` carries

---

## Open questions

1. **`quick-xml` Writer API** — the current OOXML reader uses
   `quick-xml::Reader`. The writer API (`quick-xml::Writer`) is in the same
   crate but is a different surface. Confirm that `quick-xml` ≥ 0.36 (the
   version in use) exposes a sufficiently ergonomic `Writer::write_event` API
   for structured XML output before committing to it.

2. **Style name escaping for ODF** — ODF style names that contain spaces
   (e.g. `"Heading 1"`) are URL-encoded by LibreOffice to `"Heading_20_1"`.
   The import reader already records decoded names; the exporter must re-encode
   them (or store the original name in `NodeAttr`) to preserve round-trip
   fidelity for documents from LibreOffice. ADR-0002 mentions this implicitly
   but does not specify the encoding rule. Needs a concrete decision before
   Step 6.

3. **Numbering definition generation for OOXML** — `BulletList` and
   `OrderedList` import to `Block` with per-paragraph `list_id`/`list_level`
   injected onto `ParaProps`. On export, `word/numbering.xml` must define an
   abstract numbering and a concrete numbering instance for each unique
   `list_id`. The mapping from `loki-doc-model` list attributes to OOXML
   `<w:abstractNum>` / `<w:num>` is non-trivial; if it blocks v0.1 ship, lists
   should be deferred and emitted as plain paragraphs with a bullet character.

4. **Image embedding** — `Inline::Image` and `Block::Figure` require writing
   image bytes into `word/media/` (OOXML) or `Pictures/` (ODF), adding
   relationship entries, and emitting `<w:drawing>` / `draw:frame` XML. This
   is explicitly out of scope for v0.1 but should be tracked as the primary
   v0.2 feature given that images appear in most real documents.

5. **`DocumentSettings` defaults** — `Document.settings` is `Option<DocumentSettings>`.
   When `None`, the exporter should emit the format's default value
   (`<w:defaultTabStop w:val="720"/>` for OOXML; settings.xml may be omitted
   for ODF if no document-wide settings are present). Confirm that Word and
   LibreOffice treat a missing `settings.xml` as an error or silently use
   defaults.
