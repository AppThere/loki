# Loki Layout & Rendering Fidelity Status

This is the living source of truth documenting which document features, character/paragraph properties, table features, and document structural items are supported by Loki's import, layout, rendering, and export pipelines.

---

## 1. Document Structure & Pagination

| Feature | Import Supported? | Layout / Render Supported? | Export Supported? | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **Page Size & Margins** | Yes | Yes | Yes | Page sizes (A4, Letter) and margins resolved and written. |
| **Section Breaks** | Yes | Yes | Yes | Supports multiple sections with different page layouts. |
| **Multi-column Sections** | Yes | — | Yes | Column count, gap, and separator line round-trip in both formats — DOCX `w:cols` (`@w:num`/`@w:space`/`@w:sep`) and ODF `style:columns` (`fo:column-count`/`fo:column-gap` + `style:column-sep`). Tested by `export_columns.rs` (DOCX) and `multi_column_section_round_trips` (ODT). Not yet applied at layout time (text still flows single-column). |
| **Headers & Footers** | Yes | Yes | Yes | DOCX export writes all three variants — default, first-page (`w:titlePg`), and even-page (`word/settings.xml` with `w:evenAndOddHeaders`) — as `w:hdr`/`w:ftr` parts with full block content; ODT export writes them into the master page. Round-trip tested (`even_page_headers_footers_round_trip`). |
| **Footnotes & Endnotes** | Yes | Yes | Yes | Rendered at end of section with separator rules. |
| **Dynamic Fields** | Yes | Partial | Yes | DOCX export writes body-text fields as OOXML complex fields (`w:fldChar`/`w:instrText`, with `separate`+result when a snapshot is cached) — PAGE, NUMPAGES, DATE/TIME (incl. `\@` format switch), TITLE, AUTHOR, SUBJECT, FILENAME, NUMWORDS, REF/PAGEREF cross-references, and `Raw` instructions round-trip (`export_fields.rs`). PAGE / NUMPAGES in headers & footers render real per-page values (per-page re-layout in `assign_headers_footers`); body-text fields still *render* snapshots (post-layout computation, ADR-0008, is proposed). |
| **Line-boundary Splitting** | — | Yes | — | Paragraphs split cleanly across pages using `ClippedGroup` masks. |
| **keep-together** | Yes | Yes | Yes | Prevents paragraph line breaks across pages when enabled. |
| **keep-with-next** | Yes | Yes | Yes | Scans forward up to 5 blocks to place headings and body together. |
| **Widow/Orphan Control** | Yes | No | No | Ignored at layout time (Word defaults to "on"). |
| **Bookmarks** | Yes | No | Yes | Bookmarks are written/parsed but do not affect layout. |
| **Templates (DOTX / OTT)** | Yes | — | Partial | Office `.dotx`/`.dotm` and LibreOffice `.ott` open as new untitled documents (the importers key off the `officeDocument` relationship / accepted template mimetype). Export: **Save as Template** writes `.dotx` (template content type) via `DocxTemplateExport`. Five templates (Markdown, APA, MLA, Screenplay, Resume) ship as bundled `.dotx` assets (`loki-templates`) and open from the home gallery. |
| **ODT export** | Yes | — | Yes | `loki-odf`'s `OdtExport` writes a full ODT package (`content.xml` / `styles.xml` / `meta.xml` + `Pictures/`). **Lossless** for: the complete character-property set (fonts incl. complex/East-Asian, size, weight, italic, underline, strike, caps, outline, shadow, super/sub, colour, letter/word spacing, kerning, scale, languages), the complete paragraph-property set (alignment, indents, spacing, line height, keep/widow/orphan/break flags, borders, padding, tab stops, bidi, background), the named style catalog, **multi-section page geometry** (each section gets its own `style:page-layout` + `style:master-page`; section breaks are emitted as `style:master-page-name` on the first paragraph of each section, the form the importer reads back), **headers/footers** (default/first/even, written per-section into the master page with their own automatic styles + images), headings, styled paragraphs, lists, tables, footnotes/endnotes, links, **bookmarks, fields, and embedded images** (decoded from data URIs and written as `Pictures/` parts), and core Dublin Core metadata. A property-level round-trip test asserts each survives. Editing an opened `.odt` and saving round-trips to ODT. **Multi-column sections** (`style:columns` with count/gap/separator) also round-trip. Still not emitted: extended Dublin Core (publisher/contributors/…), math, comments, and the OTT template content type. |
| **Reflow (non-paginated) view** | — | Yes | — | `LayoutMode::Reflow` + `RenderMode::Reflow` render a continuous web-style flow through the same layout/Vello pipeline as paginated view (full font/size/alignment fidelity), sliced into zero-gap GPU band tiles (768pt ⇒ exact 1024 CSS px, so tiles stack seamlessly). Relayouts to the window width on resize (shell re-emits `onscroll` for scroll containers). Content wider than the viewport (e.g. a fixed-width table) widens the tiles so it is reachable by horizontal scrolling rather than clipped. No headers/footers/page chrome by design; the status-bar page indicator is hidden in reflow. **Editing:** `ContinuousLayout` carries per-paragraph editing data, so click-to-cursor, caret placement/painting, range-selection highlighting (mouse drag-select + Shift+Arrow), and reflow-native arrow / Home / End navigation all work, plus typing/undo/formatting. Still missing: typing/Backspace over a selection does not yet delete the selected range first (it inserts at the focus), and touch long-press selection is not wired for reflow. Android CPU builds (no `android_gpu`) fall back to a low-fidelity HTML flow (`reflow_view.rs`) with no caret. |

---

## 2. Character Properties

| Property | Import | Layout/Render | Export | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **Bold / Italic** | Yes | Yes | Yes | Fully supported. |
| **Numeric Font Weight** | Partial | Yes | Partial | `CharProps.font_weight` (OpenType 1–1000) renders via Parley `FontWeight`, superseding boolean `bold` when set. Editable per-style in the style editor's weight selector (Thin…Black). Import: ODF `fo:font-weight` numeric; OOXML `w:b` is boolean only. Export: DOCX collapses to bold/not-bold (≥ 600 ⇒ bold). |
| **Underline** | Yes | Yes | Yes | Style varieties (single, double, dotted, dash, wave, thick) mapped. |
| **Strikethrough** | Yes | Yes | Yes | Single and double strikethrough variants mapped. |
| **Font Family / Size** | Yes | Yes | Yes | Resolves against style catalog and font resources. |
| **Vertical Alignment** | Yes | Yes | Yes | Superscript and subscript support. |
| **Color** | Yes | Yes | Yes | Linear sRGB, transparent, and fallback mappings. |
| **Highlight Color** | Yes | Yes | Yes | Mapped to 16 standard palette colors. |
| **Letter Spacing** | Yes | Yes | Yes | Mapped to Parley letter spacing. |
| **Word Spacing** | Yes | Yes | Yes | Mapped to Parley word spacing. |
| **Small Caps / All Caps** | Yes | Yes | Yes | Uppercases characters if all-caps enabled; maps to Parley. |
| **Shadow Text** | Yes | Yes | Yes | Mapped to StyleSpan properties. |
| **Scale / Kerning** | Yes | No | No | Dropped at layout time. |
| **Language Tags** | Yes | No | No | No locale-sensitive shaping or hyphenation. |

---

## 3. Paragraph Properties

| Property | Import | Layout/Render | Export | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **Alignment** | Yes | Yes | Yes | Left, center, right, and justified alignments supported. |
| **Indentation** | Yes | Yes | Yes | Left, right, first line, and hanging indents mapped. |
| **Spacing (Before/After)** | Yes | Yes | Yes | Margins and paragraph offsets respected. |
| **Line Height** | Yes | Yes | Yes | Supports exact, relative (multipliers), and at-least rules. |
| **Borders** | Yes | Yes | Yes | Top, bottom, left, right borders supported. |
| **Tab Stops** | Yes | Yes | Yes | Position-sorted tab stops supported. |
| **Background Color** | Yes | Yes | Yes | Paragraph background shading supported. |
| **Named / Custom Paragraph Styles** | Yes | Yes | Yes | The full style catalog round-trips through DOCX `styles.xml`: custom styles created in the style editor, plus edits to built-in `Normal`/`Heading1`–`6`, persist across save/reload. Font family, weight (→ bold), size, alignment, indentation (incl. first-line), spacing, line height, `basedOn`, `next`-style, outline level, and the custom flag are all written and read back. In-session, the catalog round-trips through the Loro CRDT (`loro_bridge::styles`, a JSON snapshot like metadata), so style-editor edits are durable across rebuilds and **undoable** with Ctrl+Z/Ctrl+Y. |
| **border_between** | Yes | No | No | Rules between adjacent same-styled paragraphs ignored. |
| **bidi / RTL** | Yes | No | No | RTL paragraphs ignored due to Parley API limitations. |

---

## 4. Tables & Images

| Feature | Import | Layout/Render | Export | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **Column Widths** | Yes | Yes | Yes | Mapped using `ColWidth` and `TableWidth` specs. |
| **Row Heights** | Yes | Yes | Yes | Evaluated dynamically based on cell content. |
| **Column Spanning** | Yes | Yes | Yes | Supports horizontal cell merging. |
| **Row Spanning** | Yes | Yes | Yes | Spanning heights distributed across spanned rows (`w:vMerge`). |
| **Text Direction** | Yes | Yes | Yes | Mapped for vertical and rotated cell text. |
| **Inline Images** | Yes | Yes | Yes | Positioned inline drawings rendered via data URIs. |
| **External Images** | Yes | No | No | Renders as gray placeholder rectangles. |

---

## 5. Target Applications & Compatibility Policy

To ensure high-fidelity visual rendering, Loki compares its output against the following reference applications:
*   **OOXML (DOCX)**: Mapped and validated against **Microsoft Word / Office 365** output.
*   **ODF (ODT)**: Mapped and validated against **LibreOffice Writer** output.

### Conflict Resolution Policy
In the event that Microsoft Word and LibreOffice exhibit mutually exclusive layout behaviors for the same document properties, **Loki prioritizes compatibility with Microsoft Word** due to its widespread adoption.

---

## 6. Font Availability & Fallback Policy

Loki implements a comprehensive strategy for font availability, dynamic loading, and fallback mapping to ensure visual fidelity across platforms (Desktop, Android, and iOS).

### Font Acquisition & Installation
- **Desktop Platforms**: The installer ensures system-wide availability of open-source fonts, including Atkinson Hyperlegible Next.
- **Mobile Platforms (Android/iOS)**: Required fonts are bundled directly inside the application package.
- **Supplied Fonts**:
  - *Atkinson Hyperlegible Next* (custom brand font)
  - *Arimo*, *Cousine*, *Tinos*, *Carlito*, *Caladea* (Google's metric-compatible replacements for common Microsoft Office fonts)
  - *Noto Sans* (fallback to prevent "Tofu" characters for unsupported glyphs)

### Metric-Compatible Font Mapping
When a document requests a font that is unavailable locally, Loki automatically substitutes it with a metric-compatible alternative:
- **Arial** &rarr; **Arimo**
- **Courier New** &rarr; **Cousine**
- **Times New Roman** &rarr; **Tinos**
- **Calibri** &rarr; **Carlito**
- **Cambria** &rarr; **Caladea**

### Font Substitution Alerts
When any font substitutions or missing fonts (such as **Aptos**, which has no open-source metric-compatible fallback) are detected in a document, Loki:
1. Flags the substitution/missing font status in the document's layout context.
2. Displays a premium, dismissible UI warning banner at the bottom of the editor viewport (above the status bar/ribbon) containing details of the substituted fonts.
3. Provides official download links for Microsoft proprietary fonts (e.g. Aptos) to help users obtain the original fonts for high-fidelity rendering.

---

## 7. Import Security Limits (Hostile-Input Hardening)

ODF/OPC import enforces hard limits against crafted hostile files (audit
2026-06-10, S1–S4). Legitimate documents are unaffected; values at or beyond
these limits are clamped or rejected with a typed error.

| Limit | Value | Behaviour when exceeded |
| :--- | :--- | :--- |
| Decompressed size per ZIP entry (loki-opc, loki-odf) | 256 MiB | `EntryTooLarge` error |
| Aggregate decompressed size per package (loki-opc, loki-odf) | 1 GiB | `PackageTooLarge` error |
| ODS materialized cells per repeat axis (`number-rows/columns-repeated`) | 10,000 | Clamped; row/column cursors still advance by the full sheet-clamped repeat (1,048,576 rows / 16,384 cols) |
| ODT table columns expanded per `table:table-column` repeat | 16,384 | Clamped |
| ODT spaces per `<text:s text:c="N"/>` | 10,000 | Clamped |
| ODT nesting depth (`text:span` / `text:a` / `text:list`) | 100 | `NestingTooDeep` error |

---

## 8. Layout Performance (Editing Path)

These are performance characteristics, not fidelity changes — layout output is
byte-identical with and without the caches below. See
[benchmarks.md](file:///home/user/loki/docs/benchmarks.md) for the harness and
numbers.

| Mechanism | Where | Effect |
| :--- | :--- | :--- |
| Paragraph shaping cache (`para_cache`) | `loki-layout` `FontResources` | Re-shapes only the changed paragraph per keystroke; unchanged paragraphs are served from cache. Keystroke cost on a ~1000-paragraph doc dropped ~166 ms → ~22 ms. Keyed by a hash of every shaping input (text + `Debug` of spans/props + width + scale + preserve flag), bounded by a two-generation LRU (`CACHE_CAP` = 4096). |
| Persistent renderer shaping context | `loki-renderer` `DocPageSource` | One `FontResources` for the source's lifetime instead of one per generation: the ~20 MB system-font scan runs once, and the shaping cache persists across keystrokes on the render path. |
| Incremental Loro→Document reconstruction (`IncrementalReader`) | `loki-doc-model` `loro_bridge` | A keystroke re-derives only the changed block instead of walking the whole CRDT, by diffing Loro versions and mapping changed containers to block indices. Structural/section/layout changes fall back to a full `loro_to_document`, so output is byte-identical. Keystroke on a ~1000-paragraph doc: full-rebuild ~17.7 ms → incremental ~14 ms (~166 ms → ~14 ms across both Tier-0 commits). |
| Single canonical layout | `loki-renderer` `DocPageSource` / `DocumentView` | Paginated mode lays the document out once: the editor's `Arc<PaginatedLayout>` is handed to the renderer (`provide_paginated_layout`) and reused for painting instead of a second `layout_document` pass. Reflow mode still computes its own width-dependent layout. The editor (hit-testing) and renderer (painting) now share one layout object. Output unchanged. |
| Lazy renderer font context | `loki-renderer` `DocPageSource` | `layout_resources` (the ~20 MB system-font scan) is now built lazily. In paginated mode the renderer reuses the editor's provided layout and never lays out, so it no longer pays a redundant per-open font scan (~7–25 ms saved at open). Only built when the renderer actually lays out (reflow, or paginated without a provided layout). |
| Open → first-paint benchmark | `loki-acid` `examples/load_bench` | Times the open pipeline on a 19-page DOCX (release): font scan ≈ 7–25 ms, import ≈ 2 ms, `document_to_loro` ≈ 2 ms, first `layout_document` ≈ 30–73 ms (one-time glyph shaping dominates). The editor shows a blank-page **loading indicator** (`editor-document-loading`) immediately on tab open so the async read/import is masked. |

---

## 9. Publishing Export (PDF/X & EPUB 3.3)

The **Publish** ribbon tab in Loki Text exports the open document to print-ready
PDF/X and to reflowable EPUB 3.3, and edits Dublin Core metadata. PDF export
(`loki-pdf`) reuses the shared `loki-layout` engine to reproduce the document's
own paginated geometry, then serialises positioned glyph runs with `pdf-writer`.
EPUB export (`loki-epub`) serialises the abstract content tree to XHTML inside an
OCF (ZIP) container.

| Feature | Source | Status | Notes |
| :--- | :--- | :---: | :--- |
| **PDF/X-1a / X-3 / X-4** | `loki-pdf` | Yes | Conformance level selectable per export. Drives PDF version (1.4 / 1.4 / 1.6), the `GTS_PDFXVersion` Info key, and the XMP `pdfx`/`pdfxid` claim. |
| **Font embedding** | `loki-pdf` | Yes | Every face used by the layout is embedded as a `CIDFontType2` program (`Identity-H`), glyphs addressed by id. Full program embedded; **subsetting is deferred** (larger files). |
| **CMYK colour + OutputIntent** | `loki-pdf` | Yes | All text/graphics emitted in DeviceCMYK with a PDF/X `OutputIntent`. Default printing condition is FOGRA39. An ICC `DestOutputProfile` is embedded only when supplied via `OutputIntent::icc_profile`; otherwise the registered condition identifier is referenced (supply a licensed profile for full certification). |
| **XMP + Info + trailer ID + Trapped** | `loki-pdf` | Yes | XMP metadata packet, Document Info dictionary, trailer `/ID`, and `Trapped` flag all written (PDF/X requirements). |
| **Text decorations / rules / borders / fills** | `loki-pdf` | Yes | Underline/strikethrough/overline, horizontal rules, table borders and cell fills emitted as CMYK fills. |
| **Images in PDF** | `loki-pdf` | Yes | `data:` URI images are decoded, converted to **DeviceCMYK**, Flate-compressed, and embedded as image XObjects; transparency is preserved via a DeviceGray soft mask. CMYK conversion is the naive transform (no ICC); subsetting/recompression of already-CMYK sources is not yet optimised. |
| **Clipping / rotation in PDF** | `loki-pdf` | Partial | `ClippedGroup` renders children without the clip mask; `RotatedGroup` renders at the group origin without rotation (over-paint preferred to omission). |
| **EPUB 3.3 container** | `loki-epub` | Yes | OCF ZIP with `mimetype` stored first, `META-INF/container.xml`, package document, navigation document, one XHTML content document, a stylesheet, and packaged image resources. |
| **EPUB package metadata** | `loki-epub` | Yes | Required `dc:identifier` (synthesised UUID when absent) / `dc:title` / `dc:language` / `dcterms:modified`, plus all available Dublin Core fields. |
| **EPUB content** | `loki-epub` | Yes | Paragraphs, headings (with a derived TOC `nav`), lists, blockquotes, code, rules, definition lists, **tables** (`<thead>`/`<tbody>`/`<tfoot>` with `colspan`/`rowspan`), inline formatting, and **images** (`data:` URIs packaged as `EPUB/images/*` resources and listed in the manifest; external URLs referenced in place). Math, fields, and comments are dropped. |
| **Dublin Core metadata editor** | `loki-text` | Yes | Publish-tab **Metadata** button edits the DCMES + DCMI Terms fields (`DocumentMeta` + `DublinCoreMeta`). Edits are persisted **through the Loro CRDT** (`loro_bridge::write_document_meta`, stored as a JSON snapshot under the metadata map), so they survive incremental rebuilds, participate in undo/redo, and round-trip through Loro import/export. Metadata is **not** yet written back to DOCX/ODT on export. |

---

## 10. ACID Fidelity Test Harness (`loki-acid`)

The `loki-acid` crate operationalises the ACID rendering test plan
([`loki-acid/TEST_PLAN.md`](file:///home/user/loki/loki-acid/TEST_PLAN.md)): a
machine-readable catalog of 139 constructs (`TC-*`) that alternative office
suites render differently from the canonical Microsoft 365 (OOXML) / LibreOffice
(ODF) render, plus the harness to diff Loki against golden references.

| Layer | Status | Notes |
| :--- | :---: | :--- |
| **Case catalog** | Yes | All 139 cases (DOCX 38, XLSX 30, PPTX 29, ODT 14, ODP 9, ODG 9, ODS 10) transcribed with severity + format. |
| **Fixtures** | Yes | `acid_docx/odt/xlsx/ods/odp/odg` embedded via `include_bytes!`. PPTX fixture not yet supplied. |
| **Page-count + glyph-coverage canaries** | Yes | Computed from the layout (no GPU). `cargo run -p loki-acid --example acid_report` imports every fixture and reports page/sheet counts and tofu (`.notdef`) pages. |
| **SSIM / pixel diff** | Scaffolded | Pure, unit-tested SSIM + abs-diff metrics and golden discovery (`goldens/<stem>/page-NNN.png` ↔ `renders/<stem>/page-NNN.png`). The pixel test is a documented no-op until both trees are populated. |
| **Loki headless raster** | Pending | Loki's renderer is GPU-backed; producing `renders/*` headlessly (GPU runner or a future `vello_cpu` path) is the one remaining wiring step. |
| **ODF round-trip structural diff** | Pending | Tracked alongside the ODP/ODG importers and ODS/ODP/ODG export. |

Importer coverage exercised by the harness today: DOCX + ODT (import → paginate →
glyph coverage), XLSX + ODS (import → workbook). ODP/ODG have no importer yet and
are catalogued as pending.
