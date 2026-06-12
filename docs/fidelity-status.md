# Loki Layout & Rendering Fidelity Status

This is the living source of truth documenting which document features, character/paragraph properties, table features, and document structural items are supported by Loki's import, layout, rendering, and export pipelines.

---

## 1. Document Structure & Pagination

| Feature | Import Supported? | Layout / Render Supported? | Export Supported? | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **Page Size & Margins** | Yes | Yes | Yes | Page sizes (A4, Letter) and margins resolved and written. |
| **Section Breaks** | Yes | Yes | Yes | Supports multiple sections with different page layouts. |
| **Headers & Footers** | Yes | Yes | Partial | Dynamic assignment supported; export lacks full plumbing. |
| **Footnotes & Endnotes** | Yes | Yes | Yes | Rendered at end of section with separator rules. |
| **Dynamic Fields** | Yes | Partial | No | PAGE / NUMPAGES fields in headers & footers render real per-page values (per-page re-layout in `assign_headers_footers`). Body-text fields still render snapshots. Post-layout computation (ADR-0008) is proposed. |
| **Line-boundary Splitting** | — | Yes | — | Paragraphs split cleanly across pages using `ClippedGroup` masks. |
| **keep-together** | Yes | Yes | Yes | Prevents paragraph line breaks across pages when enabled. |
| **keep-with-next** | Yes | Yes | Yes | Scans forward up to 5 blocks to place headings and body together. |
| **Widow/Orphan Control** | Yes | No | No | Ignored at layout time (Word defaults to "on"). |
| **Bookmarks** | Yes | No | Yes | Bookmarks are written/parsed but do not affect layout. |
| **Reflow (non-paginated) view** | — | Yes | — | `LayoutMode::Reflow` + `RenderMode::Reflow` render a continuous web-style flow through the same layout/Vello pipeline as paginated view (full font/size/alignment fidelity), sliced into zero-gap GPU band tiles (768pt ⇒ exact 1024 CSS px, so tiles stack seamlessly). Relayouts to the window width on resize (shell re-emits `onscroll` for scroll containers). Content wider than the viewport (e.g. a fixed-width table) widens the tiles so it is reachable by horizontal scrolling rather than clipped. No headers/footers/page chrome by design; the status-bar page indicator is hidden in reflow. **Editing:** `ContinuousLayout` now carries per-paragraph editing data, so click-to-cursor (hit testing) and caret placement/painting work in reflow, plus typing/undo/formatting. Still missing: range-selection highlight in reflow, and arrow-key navigation uses the *paginated* line geometry (functional but can step oddly relative to reflowed lines). Android CPU builds (no `android_gpu`) fall back to a low-fidelity HTML flow (`reflow_view.rs`) with no caret. |

---

## 2. Character Properties

| Property | Import | Layout/Render | Export | Notes |
| :--- | :---: | :---: | :---: | :--- |
| **Bold / Italic** | Yes | Yes | Yes | Fully supported. |
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
| **border_between** | Yes | No | No | Rules between adjacent same-styled paragraphs ignored. |
| **bidi / RTL** | Yes | No | No | RTL paragraphs ignored due to Parley 0.6 API limitations. |

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
