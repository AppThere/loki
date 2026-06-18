# Loki ACID Rendering Test Suite — Master Test Plan

**Purpose.** Each generated document is an *acid test* for office-document rendering
fidelity. Every test case targets a construct that office-suite alternatives
(LibreOffice, OnlyOffice, Google Docs/Sheets/Slides, Apple Pages/Numbers/Keynote,
Collabora, WPS) are *known* to render differently from the canonical Microsoft 365 /
Office desktop render. The suite is the reference input for Loki's visual-diff harness:
render each document in Loki, render the same document in O365 (canonical), and diff.

**How to use.**
1. Open the canonical O365 render of each file → save as PDF/PNG reference.
2. Render the same file in Loki → diff against the reference.
3. Each in-document section is labelled with its test-case ID (e.g. `TC-DOCX-014`).
   A failure on that page means Loki diverges on that feature.

**Severity.**
- **P0** — silent data/layout corruption a reader will notice immediately (wrong merge,
  dropped text, wrong page count, garbled glyphs).
- **P1** — visible fidelity gap (wrong spacing, wrong colour, wrong wrap) that a careful
  reader catches.
- **P2** — subtle metric/typographic drift.

**Canonical source of truth = Microsoft 365 desktop render** unless a case explicitly
tests the *producing* application's native format (ODF cases are diffed against
LibreOffice's own canonical ODF render, since ODF has no Microsoft canonical render —
see the ODF note in each ODF section).

---

## 1. DOCX — Word-processing rendering (`acid_docx.docx`)

| ID | Feature | What it exercises / canonical O365 behaviour | Common alt-suite failure | Sev |
|----|---------|----------------------------------------------|--------------------------|-----|
| TC-DOCX-001 | Line spacing `lineRule="auto"` | `w:line="360"` with `auto` = 1.5× the *computed* line height, a 240ths multiplier, NOT twips. | LibreOffice/old importers treat 360 as twips → lines too tight/loose; reflow + page count drift. | P0 |
| TC-DOCX-002 | Line spacing `atLeast` vs `exact` | `atLeast` grows for tall glyphs/inline images; `exact` clips. | Suites that treat `exact` as `atLeast` over-grow rows; clipped superscripts re-appear. | P1 |
| TC-DOCX-003 | Table vertical merge (`vMerge`) | Two-pass restart/continue; a `<w:vMerge/>` (no `val`) continues the cell above. | Naive importers drop the continue cells → row collapse, text duplication, border gaps. | P0 |
| TC-DOCX-004 | Table horizontal merge (`gridSpan`) | Cell spans N grid columns; borders resolve across the span. | Off-by-one column mapping; spanned borders double-drawn. | P1 |
| TC-DOCX-005 | Combined `vMerge`+`gridSpan` | L-shaped / cross merges (the must-not-split two-pass case). | Cell geometry collapses; content lands in the wrong cell. | P0 |
| TC-DOCX-006 | Table layout: `autofit` vs `fixed` | `tblLayout type="fixed"` honours `gridCol`; `autofit` re-flows to content. | Alt suites force autofit → column widths drift, page-width overflow. | P1 |
| TC-DOCX-007 | Nested tables | Table inside a cell; inner cell margins independent. | Inner table inherits outer borders; padding lost. | P1 |
| TC-DOCX-008 | Tab stops + leaders | Left/center/right/decimal/bar stops; dot/underscore leaders. | Decimal alignment falls back to left; leaders missing; bar tab absent. | P1 |
| TC-DOCX-009 | Decimal tab alignment | Numbers align on the locale decimal separator at the stop. | Aligns on first digit or right edge instead. | P1 |
| TC-DOCX-010 | Multilevel list numbering | `w:numbering` overrides, level restart, `lvlText="%1.%2"`. | Restart-at fails; legal numbering shows wrong parent; indents wrong. | P0 |
| TC-DOCX-011 | List restart + `startOverride` | Numbering restarts at a defined value mid-document. | Continues old count → 11,12 instead of 1,2. | P1 |
| TC-DOCX-012 | Custom bullet glyphs (Symbol/Wingdings) | Bullet uses a `Symbol`-font codepoint via `w:sym`. | Glyph rendered from wrong font → box/garbage; bullet missing. | P1 |
| TC-DOCX-013 | Paragraph borders + shading | `pBdr` box, partial borders, `shd` pattern fills. | Border-conflict resolution wrong; pattern shading flattened to solid. | P1 |
| TC-DOCX-014 | Character shading / highlight | `w:highlight` (16 named) vs `w:shd` run shading — different rendering. | Both collapsed to one; highlight colour wrong. | P2 |
| TC-DOCX-015 | Drop cap (in-margin / dropped) | `w:framePr dropCap="drop"` lines spanned; text wraps around. | Drop cap rendered inline at body size; wrap lost. | P1 |
| TC-DOCX-016 | Columns + column balancing | Multi-column section, unequal widths, `w:cols equalWidth="0"`. | Forces equal columns; balance ignored; break in wrong place. | P1 |
| TC-DOCX-017 | Section breaks (page sizes) | Mixed portrait/landscape sections; per-section margins. | Orientation reset to first section; margins lost. | P0 |
| TC-DOCX-018 | Headers/footers: first/odd/even | `titlePg`, `evenAndOddHeaders`; different content per type. | First-page header leaks to all; even/odd ignored. | P1 |
| TC-DOCX-019 | Page-number fields + restart | `PAGE`/`NUMPAGES`, `sectPr` page-number restart + format (roman). | Restart ignored; roman numerals show arabic. | P1 |
| TC-DOCX-020 | Footnotes + endnotes | Separator, continuation, numbering format, bottom-anchoring. | Footnote floats mid-page; endnotes inlined; numbering off. | P1 |
| TC-DOCX-021 | Cross-references / bookmarks | `REF`/`PAGEREF` to a bookmark; unique `w:id` per bookmark. | Duplicate bookmark ids (the `w:id="1"` trap) → garbled refs. | P0 |
| TC-DOCX-022 | Hyperlinks (internal + external) | `w:hyperlink anchor=` vs `r:id=`; visited styling. | Internal anchor not resolved; styling lost. | P2 |
| TC-DOCX-023 | Floating image wrap modes | `wrapSquare`/`wrapTight`/`wrapThrough`/`topAndBottom`; `wrapPolygon`. | All collapse to inline or square; text overlaps image. | P0 |
| TC-DOCX-024 | Image anchor + position | `positionH relativeFrom=page/margin/column`; absolute offsets. | Anchor reflows with text; absolute position lost → image jumps. | P1 |
| TC-DOCX-025 | Text box / shape with text | `mc:AlternateContent` DrawingML textbox; vertical text. | AlternateContent fallback chosen wrong; vertical text horizontal. | P1 |
| TC-DOCX-026 | OMML math equations | `m:oMathPara`: fractions, radicals, n-ary, matrices, accents. | Math rendered as linear text; stretchy brackets fixed-size. | P1 |
| TC-DOCX-027 | Font fallback / substitution | Document declares a font absent on the box; metric substitution. | Non-metric substitute → reflow, page-count change, overflow. | P0 |
| TC-DOCX-028 | East-Asian + Latin font split | `w:rFonts ascii=/eastAsia=` dual fonts in one run. | EastAsia font ignored → CJK in Latin font (tofu). | P1 |
| TC-DOCX-029 | RTL / bidi paragraph | `w:bidi`, mixed Hebrew/Arabic + Latin + digits; mirrored punctuation. | Run order reversed; neutrals on wrong side; digits LTR-broken. | P0 |
| TC-DOCX-030 | Character spacing / kerning / scale | `w:spacing` (expand), `w:kern`, `w:w` (horizontal scale), `w:position`. | Tracking ignored; scale applied to size; baseline shift lost. | P2 |
| TC-DOCX-031 | Small caps vs all caps | `w:smallCaps` (true small caps metrics) vs `w:caps`. | Small caps faked by scaling; metrics differ. | P2 |
| TC-DOCX-032 | Tracked changes display | `w:ins`/`w:del` rendered with author colour + strikethrough. | Changes silently accepted; author colours collapsed. | P1 |
| TC-DOCX-033 | Comments / ranges | `commentRangeStart/End` highlight; threaded replies. | Range highlight misplaced; replies flattened. | P2 |
| TC-DOCX-034 | Content controls (SDT) | Date picker, dropdown, plain-text SDT placeholder text. | Placeholder text shown as literal; control chrome lost. | P2 |
| TC-DOCX-035 | Watermark | Header `VML`/DrawingML washout text behind body. | Watermark on top of text or missing; wrong opacity. | P1 |
| TC-DOCX-036 | Theme colour resolution | Run colour `themeColor="accent1" themeTint`/`themeShade`. | Theme colour resolved to default; tint/shade math wrong. | P1 |
| TC-DOCX-037 | `keepNext`/`keepLines`/`widowControl` | Pagination: heading kept with next; no orphan/widow lines. | Heading orphaned at page bottom; widow lines appear. | P1 |
| TC-DOCX-038 | Hanging indent + first-line indent | `w:ind firstLine` vs `hanging`; negative indents. | Hanging flips to first-line; negative clipped at margin. | P2 |

## 2. XLSX — Spreadsheet rendering (`acid_xlsx.xlsx`)

| ID | Feature | What it exercises / canonical O365 behaviour | Common alt-suite failure | Sev |
|----|---------|----------------------------------------------|--------------------------|-----|
| TC-XLSX-001 | Custom number format sections | `pos;neg;zero;text` 4-section codes; colour codes `[Red]`. | Section/colour parsing wrong; text section dropped. | P1 |
| TC-XLSX-002 | Accounting format alignment | `_($* #,##0.00_)` — symbol left, digits right, `_)` pad. | Padding char ignored → misaligned columns. | P1 |
| TC-XLSX-003 | Fractions & scientific | `# ?/?`, `# ??/??`, `0.00E+00`. | Fraction shown as decimal; exponent format wrong. | P2 |
| TC-XLSX-004 | Locale-dependent date/time | `[$-409]mmmm`, `[$-40C]`, elapsed `[h]:mm:ss`. | Locale token ignored; elapsed clamps at 24h. | P1 |
| TC-XLSX-005 | Conditional format: colour scale | 3-colour scale with min/mid/max + percentile. | Interpolation off; midpoint colour wrong. | P1 |
| TC-XLSX-006 | Conditional format: data bars | Gradient + solid bars, negative axis, bar direction. | Negative-axis placement wrong; gradient flat. | P1 |
| TC-XLSX-007 | Conditional format: icon sets | 3/4/5 icon sets, reversed, "show icon only". | Wrong icon thresholds; icons missing entirely. | P1 |
| TC-XLSX-008 | Conditional format: formula rule | `=MOD(ROW(),2)=0` banding; relative refs. | Relative-ref anchoring wrong → bands shift. | P1 |
| TC-XLSX-009 | Dynamic-array spill | `=SEQUENCE`, `=SORT`, `=UNIQUE`, `=FILTER` spill range. | No spill support → `#NAME?`/single cell; ghost spill. | P0 |
| TC-XLSX-010 | Legacy CSE array formula | `{=SUM(A1:A3*B1:B3)}` array-entered. | Treated as scalar → wrong total. | P1 |
| TC-XLSX-011 | Modern functions | `XLOOKUP`, `LET`, `LAMBDA`, `TEXTJOIN`, `IFS`, `SWITCH`. | `#NAME?` on unknown functions; LAMBDA unsupported. | P0 |
| TC-XLSX-012 | Structured table references | `Table1[[#Headers],[Col]]`, `[@Col]` this-row. | Structured ref not resolved → `#REF!`. | P1 |
| TC-XLSX-013 | Merged cells + alignment | Centred-across-merge; vertical centre; wrap in merge. | Value duplicated to all merged cells; centring lost. | P1 |
| TC-XLSX-014 | Text rotation + indent | 90°, 45°, stacked vertical text; indent levels. | Rotation snapped to 0/90; stacked text horizontal. | P1 |
| TC-XLSX-015 | In-cell rich text runs | Multiple fonts/colours in one cell string. | Whole cell takes first run's format. | P1 |
| TC-XLSX-016 | Frozen + split panes | `pane` freeze at B2; split with independent scroll. | Freeze lost on import; split converted to freeze. | P2 |
| TC-XLSX-017 | Charts: combo + secondary axis | Column+line combo, secondary value axis, axis cross. | Secondary axis dropped; combo flattened to one type. | P1 |
| TC-XLSX-018 | Charts: scatter/bubble | XY scatter, bubble sizing, log axis. | Bubble size linear; log axis linear. | P2 |
| TC-XLSX-019 | Sparklines | Line/column/win-loss sparklines in a cell. | Not rendered (drop entirely). | P1 |
| TC-XLSX-020 | Data validation dropdown | List from range, input/error messages. | Dropdown chrome lost; validation not enforced. | P2 |
| TC-XLSX-021 | Defined names (scoped) | Workbook vs sheet-scoped names; relative names. | Scope collision; relative name mis-anchored. | P2 |
| TC-XLSX-022 | Cross-sheet 3-D refs | `=SUM(Jan:Dec!B2)` 3-D range. | 3-D collapse to single sheet. | P1 |
| TC-XLSX-023 | Threaded vs legacy comments | New threaded comments + legacy notes with author. | Threaded shown as note; author/date lost. | P2 |
| TC-XLSX-024 | Theme + tint cell fill | `theme=4 tint=0.4` fill, font theme colours. | Theme fill → black/white; tint math wrong. | P1 |
| TC-XLSX-025 | Border precedence | Adjacent cells competing borders; diagonal borders. | Diagonal lost; wrong winner on shared edge. | P2 |
| TC-XLSX-026 | Number precision / 1900 leap bug | `=1/3` display vs stored; the 1900 leap-year quirk. | Off-by-one date; rounding display differs. | P2 |
| TC-XLSX-027 | Hidden rows/cols + outline groups | Grouped/collapsed outline levels; hidden affecting SUBTOTAL. | Groups expanded; SUBTOTAL counts hidden. | P2 |
| TC-XLSX-028 | Print: areas + scaling + repeat | `print_area`, fit-to-page, repeat header rows. | Print area ignored; scaling lost. | P2 |
| TC-XLSX-029 | Conditional format priority/stopIfTrue | Overlapping rules, `stopIfTrue`, priority order. | Wrong rule wins; stopIfTrue ignored. | P1 |
| TC-XLSX-030 | Pivot table render | Static pivot cache; grouped fields; layout. | Pivot rendered as flat range; grouping lost. | P1 |

## 3. PPTX — Presentation rendering (`acid_pptx.pptx`)

| ID | Feature | What it exercises / canonical O365 behaviour | Common alt-suite failure | Sev |
|----|---------|----------------------------------------------|--------------------------|-----|
| TC-PPTX-001 | Master → layout → slide inheritance | Placeholder idx inheritance for pos/format. | Inheritance broken → placeholder reverts to default pos. | P0 |
| TC-PPTX-002 | Theme colour + font scheme | `srgbClr`/`schemeClr` map; major/minor fonts. | Scheme colour → black; minor font ignored. | P1 |
| TC-PPTX-003 | Text autofit: shrink | `normAutofit fontScale/lnSpcReduction` shrinks overset text. | Autofit ignored → text overflows shape. | P0 |
| TC-PPTX-004 | Text autofit: resize shape | `spAutoFit` grows shape to text. | Shape stays fixed → clipped text. | P1 |
| TC-PPTX-005 | Gradient fills | Linear/radial/path gradients, multi-stop, angle. | Gradient flattened to first stop; angle ignored. | P1 |
| TC-PPTX-006 | Picture / texture / pattern fill | Tiled picture fill, preset pattern, alpha. | Tiling lost; pattern → solid. | P1 |
| TC-PPTX-007 | Shape effects: shadow/glow/reflection | Outer shadow, glow, reflection, soft edge. | Effects dropped entirely. | P1 |
| TC-PPTX-008 | 3-D bevel + rotation | `sp3d` bevel, `scene3d` camera rotation. | Flattened to 2-D. | P2 |
| TC-PPTX-009 | Custom geometry (freeform) | `custGeom` path with curves + arcs. | Path mis-rendered; arcs → lines. | P1 |
| TC-PPTX-010 | Preset geometry adjust handles | `prstGeom` with `avLst` adjust values (rounded-rect radius). | Adjust values ignored → default geometry. | P2 |
| TC-PPTX-011 | Grouped shapes + child transform | `grpSp chOff/chExt` coordinate remap; nested groups. | Child transform math wrong → shapes scatter. | P0 |
| TC-PPTX-012 | Connectors | `cxnSp` bent/curved connector bound to shapes. | Connector endpoints detach; routing straight. | P2 |
| TC-PPTX-013 | Tables in slides | Table styles, banding, merged cells, cell fill. | Table style lost; banding → no fill. | P1 |
| TC-PPTX-014 | Embedded chart | DrawingML chart with theme + data labels. | Chart → static image or dropped. | P1 |
| TC-PPTX-015 | SmartArt | `dgm` diagram (process/cycle) with colour transform. | SmartArt → fallback group or blank. | P1 |
| TC-PPTX-016 | Entrance/exit animations | `p:timing` entrance (fade/fly), exit, build by paragraph. | All animations dropped silently. | P1 |
| TC-PPTX-017 | Emphasis + motion path | Spin/grow emphasis; custom motion-path animation. | Motion path ignored. | P2 |
| TC-PPTX-018 | Slide transitions | Morph, push, fade transitions with duration. | Transition dropped; morph → cut. | P2 |
| TC-PPTX-019 | Picture crop + effects | `srcRect` crop, recolor, artistic effect, frame. | Crop lost (full image shown); recolor ignored. | P1 |
| TC-PPTX-020 | Bullet formatting | `buChar`/`buAutoNum`/`buNone`, bullet colour/size, indent levels. | Bullet glyph wrong; per-level indent collapsed. | P1 |
| TC-PPTX-021 | Line spacing + space before/after | `lnSpc spcPct`, `spcBef/spcAft` in points vs percent. | Percent vs point confusion → wrong spacing. | P1 |
| TC-PPTX-022 | Text vertical anchor + wrap | `anchor=ctr/b`, `wrap=none`, inset margins. | Anchor → top; insets ignored. | P2 |
| TC-PPTX-023 | Vertical / rotated text | `vert="eaVert"`, `vert270`, stacked. | Vertical text horizontal. | P1 |
| TC-PPTX-024 | Hyperlinks + action buttons | `hlinkClick` to slide; action button jump. | Action lost; link to wrong slide. | P2 |
| TC-PPTX-025 | Slide number / date / footer placeholders | `sldNum`/`datetime` fields auto-fill. | Field literal `<number>` shown. | P2 |
| TC-PPTX-026 | Header/footer per layout | Footer visibility per layout; master toggles. | Footer on all slides or none. | P2 |
| TC-PPTX-027 | Embedded font subset | `embeddedFont` subset; fallback if missing. | Embedded font ignored → substitute reflow. | P1 |
| TC-PPTX-028 | Tab stops in text body | Custom `tabLst` in shape text. | Tabs collapse to default; alignment lost. | P2 |
| TC-PPTX-029 | Gradient text / WordArt | Text fill gradient, outline, `prstTxWarp`. | Text fill → solid; warp ignored. | P2 |

## 4. ODT — OpenDocument Text (`acid_odt.odt`)

> **ODF canonical note.** ODF has no Microsoft-canonical render. Diff ODT/ODP/ODG/ODS
> against the **LibreOffice** canonical render *and* check the round-trip: ODT→Loki→ODT
> should preserve these constructs, and Loki's render should match LibreOffice's.

| ID | Feature | What it exercises | Common failure | Sev |
|----|---------|-------------------|----------------|-----|
| TC-ODT-001 | Style inheritance (`style:parent-style-name`) | Paragraph/text style chains; default-style fallback. | Parent chain not walked → wrong inherited props. | P1 |
| TC-ODT-002 | `fo:line-height` variants | Percent, fixed, `style:line-height-at-least`. | At-least treated as fixed. | P1 |
| TC-ODT-003 | Table cell spanning | `table:number-rows-spanned`/`columns-spanned` + covered cells. | Covered-cell elements mis-handled → shifted content. | P0 |
| TC-ODT-004 | List styles + outline | `text:list-style` level config, outline numbering. | Level indents/format wrong. | P1 |
| TC-ODT-005 | Tab stops | `style:tab-stops` with `style:type` + leader char. | Decimal/leader lost. | P1 |
| TC-ODT-006 | Sections + columns | `text:section` with `style:columns`. | Columns ignored; section break lost. | P1 |
| TC-ODT-007 | Frames + wrap | `draw:frame` text wrap (`run-through`, `parallel`, `dynamic`). | Wrap → inline; overlap. | P1 |
| TC-ODT-008 | Master page / page layout | `style:master-page` headers/footers, page size. | Header leaks; page size reset. | P1 |
| TC-ODT-009 | Footnotes/endnotes config | `text:notes-configuration` numbering + position. | Numbering format lost. | P2 |
| TC-ODT-010 | Bibliography / fields | `text:bookmark-ref`, page-ref fields, variables. | Field shows stale/zero value. | P2 |
| TC-ODT-011 | Change tracking | `text:tracked-changes` insertions/deletions. | Changes silently accepted. | P1 |
| TC-ODT-012 | Bidi / writing-mode | `style:writing-mode="rl-tb"`; mixed scripts. | RTL run order wrong. | P1 |
| TC-ODT-013 | Drop caps | `style:drop-cap` lines + distance. | Drop cap inline. | P2 |
| TC-ODT-014 | Conditional / hidden text | `text:condition`, hidden paragraphs. | Hidden text shown. | P2 |

## 5. ODP — OpenDocument Presentation (`acid_odp.odp`)

| ID | Feature | What it exercises | Common failure | Sev |
|----|---------|-------------------|----------------|-----|
| TC-ODP-001 | Master page inheritance | `style:master-page` → `presentation:placeholder`. | Placeholder pos reverts. | P1 |
| TC-ODP-002 | Gradient / bitmap fill | `draw:gradient`, `draw:fill-image` tiling. | Gradient flat; tiling lost. | P1 |
| TC-ODP-003 | Shape effects (shadow) | `draw:shadow`, transparency. | Shadow dropped. | P2 |
| TC-ODP-004 | Text autofit | `draw:fit-to-size`, `draw:auto-grow-*`. | Overflow / clip. | P1 |
| TC-ODP-005 | Custom shapes | `draw:custom-shape` enhanced-geometry path. | Path mis-rendered. | P1 |
| TC-ODP-006 | Animations | `anim:par`/`anim:seq` effects. | Animations dropped. | P1 |
| TC-ODP-007 | Connectors | `draw:connector` glue points. | Endpoints detach. | P2 |
| TC-ODP-008 | Tables on slides | `table:table` in `draw:frame`. | Table style lost. | P1 |
| TC-ODP-009 | Slide transitions | `presentation:transition-*`. | Transition dropped. | P2 |

## 6. ODG — OpenDocument Graphics / drawing (`acid_odg.odg`)

| ID | Feature | What it exercises | Common failure | Sev |
|----|---------|-------------------|----------------|-----|
| TC-ODG-001 | Bézier paths | `draw:path` cubic/quadratic curves. | Curve → polyline. | P1 |
| TC-ODG-002 | Gradient types | linear / axial / radial / ellipsoid / square / conical. | Type confused; angle wrong. | P1 |
| TC-ODG-003 | Hatch + bitmap fill | `draw:hatch` distance/angle; bitmap tiling. | Hatch → solid. | P2 |
| TC-ODG-004 | Connectors + glue points | Standard/curved/line connectors bound to glue points. | Routing wrong. | P1 |
| TC-ODG-005 | Text along path | `draw:text-path` (Fontwork). | Text straight. | P2 |
| TC-ODG-006 | Layers | `draw:layer-set` visibility/lock. | Layer order/visibility lost. | P2 |
| TC-ODG-007 | Dimension lines | `draw:measure` with arrows + text. | Measure → plain line. | P2 |
| TC-ODG-008 | Transforms | rotate/skew/flip via `draw:transform` matrix. | Skew/flip lost. | P1 |
| TC-ODG-009 | 3-D scene | `dr3d:scene` extruded objects. | 3-D → 2-D outline. | P2 |

## 7. ODS — OpenDocument Spreadsheet (`acid_ods.ods`)

| ID | Feature | What it exercises | Common failure | Sev |
|----|---------|-------------------|----------------|-----|
| TC-ODS-001 | Data styles (number formats) | `number:number-style` sections, `number:text-style`. | Section dropped; colour lost. | P1 |
| TC-ODS-002 | `number:repeated` column/row | `table:number-columns-repeated` compression. | Repeat expanded wrong → cell shift. | P0 |
| TC-ODS-003 | Covered cells (merge) | `table:covered-table-cell` after span. | Covered cell shows content. | P0 |
| TC-ODS-004 | Conditional formats | `calcext:conditional-format` (color-scale/icon/databar). | Rule ignored. | P1 |
| TC-ODS-005 | ODF formula namespace | `of:=SUM([.A1:.A3])` ODF-syntax formulas. | ODF formula syntax not parsed → `#NAME?`. | P0 |
| TC-ODS-006 | Cell text rotation | `style:rotation-angle`, vertical stacking. | Rotation snapped. | P1 |
| TC-ODS-007 | Matrix (array) formulas | `table:number-matrix-*` array result. | Treated scalar. | P1 |
| TC-ODS-008 | Named ranges/expressions | `table:named-range`, `table:named-expression`. | Scope lost. | P2 |
| TC-ODS-009 | Cell borders + diagonal | `style:diagonal-bl-tr` etc. | Diagonal dropped. | P2 |
| TC-ODS-010 | Frozen panes / split | `table:split` config in settings. | Freeze lost. | P2 |

---

## Diff-harness hints

- **Page-count drift is the canary.** TC-DOCX-001/027 and any font-substitution case
  change pagination; assert page count equals the O365 reference *before* per-pixel diff.
- **Render at 150 DPI**, compare with a perceptual diff (SSIM) plus a hard glyph-coverage
  check (no tofu / `.notdef`).
- **Two diffs per ODF file**: (a) Loki-render vs LibreOffice-render; (b) round-trip
  structural diff (re-export and compare the XML tree for the targeted elements).
- Keep each in-document section on its **own page/slide/sheet** so a failing case maps to
  exactly one reference image.
