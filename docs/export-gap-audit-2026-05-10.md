# OOXML Export Round-Trip Gap Audit

> [!NOTE]
> This document is a historical audit snapshot from May 10, 2026. For the current, living implementation status of layout and rendering properties, please refer to the central status registry: [fidelity-status.md](file:///Users/kevin/project/loki/docs/fidelity-status.md).

**Date:** 2026-05-10
**Branch:** main

## Summary
The OOXML export implementation (Tier 3) successfully handles document structure (sections, page layout), basic block types (paragraphs, headings, lists, tables), and primary character formatting (bold, italic, size, color). However, there is a significant fidelity gap in paragraph and cell properties. Most paragraph-level layout controls (indentation variants, line height, tab stops, borders) and table structural properties (row spans, cell borders, cell shading) are imported but silently dropped during export. High-fidelity round-tripping currently fails for any document relying on precise layout or complex table structures.

## Character property gaps

| CharProps field | Imported from | Exported as | Gap? |
|----------------|---------------|-------------|------|
| bold | `w:b` | `w:b` | No |
| italic | `w:i` | `w:i` | No |
| underline | `w:u @val` | `w:u @val` | No |
| strikethrough | `w:strike`/`w:dstrike` | `w:strike`/`w:dstrike` | No |
| color | `w:color` | `w:color` | No |
| highlight_color | `w:highlight` | — | **Yes** (P1) |
| font_family | `w:rFonts` | `w:rFonts` | No (Asian/CS lost) |
| font_size_pt | `w:sz` | `w:sz` | No |
| vertical_align | `w:vertAlign` | `w:vertAlign` | No |
| letter_spacing | `w:spacing` (char) | — | **Yes** (P2) |
| word_spacing | — | — | No |
| scale | `w:w` | — | **Yes** (P2) |
| small_caps | `w:smallCaps` | `w:smallCaps` | No |
| all_caps | `w:caps` | — | **Yes** (P2) |
| shadow | `w:shadow` | — | **Yes** (P2) |
| kerning | `w:kern` | — | **Yes** (P2) |
| language | `w:lang` | — | **Yes** (P3) |
| link_url | `w:hyperlink` | `w:hyperlink` | No |

## Paragraph property gaps

| ParaProps field | Imported from | Exported as | Gap? |
|----------------|---------------|-------------|------|
| alignment | `w:jc` | `w:jc` | No |
| indent_left | `w:ind @left` | `w:ind @w:left` | No |
| indent_right | `w:ind @right` | `w:ind @w:right` | No |
| indent_first_line | `w:ind @firstLine` | — | **Yes** (P1) |
| indent_hanging | `w:ind @hanging` | `w:ind @w:hanging` | No |
| space_before_pt | `w:spacing @before` | `w:spacing @w:before` | No |
| space_after_pt | `w:spacing @after` | `w:spacing @w:after` | No |
| line_height | `w:spacing @line` | — | **Yes** (P1) |
| keep_together | `w:keepLines` | — | **Yes** (P2) |
| keep_with_next | `w:keepNext` | — | **Yes** (P2) |
| page_break_after | `w:pageBreakBefore` | — | **Yes** (P2) |
| widow_control | `w:widowControl` | — | **Yes** (P3) |
| bidi | `w:bidi` | — | **Yes** (P2) |
| border | `w:pBdr` | — | **Yes** (P1) |
| tab_stops | `w:tabs` | — | **Yes** (P1) |
| background_color | `w:shd` (para) | — | **Yes** (P1) |
| list_id / list_level | `w:numPr` | `w:numPr` | No |

## Cell property gaps

| CellProps field | Imported from | Exported as | Gap? |
|----------------|---------------|-------------|------|
| padding_top_pt | `w:tcMar/w:top` | `w:tcMar/w:top` | No |
| padding_bottom_pt | `w:tcMar/w:bottom` | `w:tcMar/w:bottom` | No |
| padding_left_pt | `w:tcMar/w:left` | `w:tcMar/w:left` | No |
| padding_right_pt | `w:tcMar/w:right` | `w:tcMar/w:right` | No |
| vertical_align | `w:vAlign` | `w:vAlign` | No |
| text_direction | `w:textDirection` | — | **Yes** (P2) |
| background_color | `w:shd @fill` | — | **Yes** (P1) |
| border_top/bottom/left/right | `w:tcBorders` | — | **Yes** (P1) |

## Table structure gaps

| Table feature | Imported from | Exported as | Gap? |
|--------------|---------------|-------------|------|
| Table width | `w:tblW` | `w:tblW` | **Partial** (Hardcoded auto) |
| Column widths | `w:tblGrid/w:gridCol` | `w:tblGrid/w:gridCol` | No |
| Col span | `w:gridSpan` | `w:gridSpan` | No |
| Row span | `w:vMerge` | — | **Yes** (P1) |
| Table borders | `w:tblBorders` | — | **Yes** (P1) |
| Header rows | `w:tblHeader` | `w:tblHeader` | No |

## Document structure gaps

| Feature | Imported from | Exported as | Gap? |
|---------|---------------|-------------|------|
| Page size | `w:pgSz` | `w:pgSz` | No |
| Page margins | `w:pgMar` | `w:pgMar` | No |
| Multiple sections | `w:sectPr` in `w:pPr` | `w:sectPr` | No |
| Default header | `w:headerReference` | — | **Yes** (P1) |
| First-page header | `w:headerReference @type="first"` | — | **Yes** (P1) |
| Even header | `w:headerReference @type="even"` | — | **Yes** (P1) |
| Footnotes | `w:footnoteReference` | `w:footnoteReference` | No |
| Endnotes | `w:endnoteReference` | `w:endnoteReference` | No |
| Field codes | `w:fldChar` / `w:fldSimple` | — | **Yes** (P1) |
| Bookmarks | `w:bookmarkStart/End` | `w:bookmarkStart/End` | No |
| Default tab stop | `w:defaultTabStop` | — | **Yes** (P2) |
| Images (data URI) | `w:drawing` | `w:drawing` | No |
| Images (external) | `w:drawing` | — | **Yes** (P1) |
| Hyperlinks | `w:hyperlink` | `w:hyperlink` | No |

## Block/inline coverage

| Block variant | Export quality | Notes |
|--------------|---------------|-------|
| Paragraph | Full | Content and basic spacing/indentation preserved. |
| Heading | Full | Promoted to standard Heading styles. |
| BulletList | Full | Mapped to `w:numPr` via `NumberingState`. |
| OrderedList | Full | Mapped to `w:numPr` via `NumberingState`. |
| Table | Partial | Structural grid and spans preserved; borders and cell props lost. |
| Figure | Full | Emitted as `w:drawing` inside a paragraph. |
| HorizontalRule | Full | Emitted as paragraph with bottom border. |
| CodeBlock | Partial | Uses "Code" style and Courier font. |
| BlockQuote | Partial | Recurses but loses block-level indentation/borders. |
| Div | Partial | Recurses but loses attributes. |
| DefinitionList | Partial | Emitted as sequence of paragraphs. |
| StyledPara | Full | Handles both style ID and direct properties. |

| Inline variant | Export quality | Notes |
|---------------|---------------|-------|
| Str | Full | Preserved as `w:t`. |
| LineBreak | Full | Emitted as `w:br`. |
| PageBreak | Missing | No variant in model; `w:br type="page"` not emitted. |
| Styled / StyledRun | Full | Handles style ID and direct properties. |
| Link | Full | Both inner text and target URL preserved. |
| Image | Full | Emitted as `w:drawing` with proper relationships. |
| Note (footnote) | Full | Emitted to `word/footnotes.xml` with proper IDs. |
| Field | Missing | Silently dropped. |
| Bookmark | Full | Emitted as `w:bookmarkStart/End`. |

## Prioritised fix list

### P0 — Data loss
- **Hyperlinks**: [RESOLVED] `Inline::Link` target URL is preserved.
- **Images**: [RESOLVED] `Inline::Image` and `Block::Figure` are emitted as `w:drawing`.
- **Notes**: [RESOLVED] Footnotes and endnotes are preserved.

### P1 — Clearly wrong
- **Table Properties**: Cell borders, background colors, and `row_span` (`w:vMerge`) are missing.
- **Paragraph Layout**: Tab stops, borders, and `line_height` are missing.
- **Headers/Footers**: Document headers and footers are not emitted in `w:sectPr`.
- **Direct Formatting**: `highlight_color` and `indent_first_line` are missing from exported `w:rPr`/`w:pPr`.

### P2 — Subtle
- **Section Breaks**: `w:sectPr` in `w:pPr` is correctly emitted for splits, but some section-level properties (beyond size/margins) are lost.
- **Font Variants**: Asian and Complex Script font family/size are lost.
- **Character Spacing**: Kerning, letter spacing, and scaling are missing.

### P3 — Cosmetic
- **Widow/Orphan Control**: `w:widowControl` is not emitted (though Word defaults to 'on').
- **Language**: `w:lang` is not emitted.

## Recommended fix order
1. **Hyperlinks**: Essential for web-sourced content.
2. **Table Structural Fidelity**: Implement `row_span` and `background_color`.
3. **Paragraph Layout**: Implement `tab_stops` and `indent_first_line` to fix common indentation issues.
4. **Images**: Crucial for most real-world documents.
5. **Headers/Footers**: Necessary for document completeness.
6. **Table/Para Borders**: Necessary for visual parity.
