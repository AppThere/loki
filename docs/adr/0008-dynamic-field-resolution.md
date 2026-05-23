# ADR 008: Dynamic Field Resolution

## Status
Proposed

## Date
2026-05-22

## Context
Document fields such as `PAGE`, `NUMPAGES`, `TOC` entries, `DATE`, `TITLE`, and `AUTHOR` currently display static snapshot values baked into the document model at import time. In the rendering output, page number fields show `"1"` regardless of the page they appear on, and Table of Contents (TOC) entries display their import-time stale text rather than reflecting the actual layout page structure.

Resolving page number fields at layout time creates a circular dependency:
1. To render a field (e.g., page number), we need to know what page it is on.
2. To know what page the field is on, we must lay out the document.
3. Laying out the document depends on the width of the field text, which changes depending on the page number (e.g. `"9"` vs. `"10"` has a different string length and shaped width).

Currently:
- [Inline::Field](file:///d:/project/loki/loki-doc-model/src/content/inline.rs#L223) carries a [Field](file:///d:/project/loki/loki-doc-model/src/content/field/types.rs#L119) struct containing `kind: FieldKind`, `current_value: Option<String>`, and `extensions: ExtensionBag`.
- [FieldKind](file:///d:/project/loki/loki-doc-model/src/content/field/types.rs#L45) includes variants for `PageNumber`, `PageCount`, `Date`, `Time`, `Title`, `Author`, `Subject`, `FileName`, `WordCount`, `CrossReference`, and `Raw`.
- In `loki-layout`, [walk_inlines](file:///d:/project/loki/loki-layout/src/resolve.rs#L677) resolves fields immediately during layout using [field_display_text](file:///d:/project/loki/loki-layout/src/resolve.rs#L705). This flattens them into plain text runs based on their `current_value` snapshot (or `"1"` for page-related fields, or `""` for others) directly into the paragraph's Parley buffer.
- Neither [PaginatedLayout](file:///d:/project/loki/loki-layout/src/result.rs#L81) nor [LayoutPage](file:///d:/project/loki/loki-layout/src/result.rs#L101) carries structural back-references to identify which blocks or inlines belong to which page, except for editing paragraph metrics in `PageEditingData`.
- `loki-vello`'s [paint_items](file:///d:/project/loki/loki-vello/src/scene.rs#L482) is stateless and has no concept of fields or runtime substitution.
- [Block::TableOfContents](file:///d:/project/loki/loki-doc-model/src/content/block.rs#L270) wraps [TableOfContentsBlock](file:///d:/project/loki/loki-doc-model/src/content/block.rs#L128) containing a stale, pre-rendered `body: Vec<Block>` snapshot without structured page references.

---

## Prior art

Four potential approaches to resolving dynamic fields were evaluated:

1. **Two-pass layout**: Lay out the document with placeholders, record page assignments, substitute resolved values, and run layout a second time.
   - *Pros*: Completely correct including line wrapping.
   - *Cons*: Doubles the layout calculation cost for every document opening and reflow. Since `loki-layout` is currently a pure, stateless pipeline, this would require running `layout_document` twice.
2. **Fixed-point iteration**: Repeatedly run layout, updating field values on each pass until page breaks and field values stabilize.
   - *Pros*: Handles highly interdependent layouts.
   - *Cons*: Extremely expensive. Without incremental layout support, it risks infinite loops if a page number change causes a layout shift that changes the page boundaries back (e.g. oscillating between 9 and 10 pages).
3. **Post-layout substitution**: Lay out once with standard-width placeholders (e.g., `"00"` for page numbers), then walk the resulting layout items, substitute the actual page numbers, re-shape only those text runs, and horizontally shift subsequent items on the same line.
   - *Pros*: Fast, single-pass layout. Prevents circular layout feedback loops.
   - *Cons*: If the substituted text width differs substantially from the placeholder, it could cause visual overlap or right-margin overflow if the line cannot wrap. However, using conservative placeholder widths (e.g. `"00"` or `"000"`) ensures the substituted text is usually smaller than or equal to the placeholder width.
4. **Deferred rendering**: Emits a placeholder variant into the Vello scene, resolving values at paint time.
   - *Pros*: Extremely fast, does not touch the layout engine.
   - *Cons*: Violates the clean separation of concerns. `loki-vello` is designed to be a stateless, render-only graphics engine. Threading document state (page counts, metadata, settings) through the paint tree makes the renderer stateful and complicated.

---

## Field classification

We classify fields into three categories by resolution phase:

| Field kind | Resolution phase | Value source | Description |
|-----------|----------------|--------------|-------------|
| `Title` | At import | `Document.meta.title` | Fixed metadata cached in `Field::current_value` |
| `Author` | At import | `Document.meta.creator` | Fixed metadata cached in `Field::current_value` |
| `Subject` | At import | `Document.meta.subject` | Fixed metadata cached in `Field::current_value` |
| `FileName` | At import | File source context | Fixed name cached in `Field::current_value` |
| `PageNumber` | Post-layout | `LayoutPage::page_number` | Resolved based on final page structure |
| `PageCount` | Post-layout | `PaginatedLayout::pages.len()` | Resolved based on final page count |
| `CrossReference` | Post-layout | Target heading position | Resolved by lookup of target block page |
| `WordCount` | Post-layout | Total block word count | Calculated by counting words across all blocks |
| `Date` | Post-layout / Paint | `SystemTime` | Evaluated using current system clock |
| `Time` | Post-layout / Paint | `SystemTime` | Evaluated using current system clock |

---

## Decisions

### 1. Resolution strategy
We choose **Approach 3: Post-layout substitution**.
Loki's layout pipeline will execute a single layout pass with configured placeholder strings. Following this, a post-layout resolution function will walk the layout items, replace placeholders with actual resolved values, and adjust subsequent items on affected lines. This maintains a fast, single-pass layout without modifying the stateless rendering boundaries of `loki-vello`.

### 2. Field classification by resolution time
The classification table outlined in the [Field classification](#field-classification) section is adopted. 
- Metadata fields (`Title`, `Author`, `Subject`, `FileName`) are resolved during file import and stored in `current_value`.
- Document metrics and page-relative fields (`PageNumber`, `PageCount`, `CrossReference`, `WordCount`) are resolved after the layout pass.
- Time-based fields (`Date`, `Time`) are resolved during the post-layout phase using the system clock.

### 3. Architecture: where resolution lives
Field resolution lives entirely in **`loki-layout`**.
We will introduce a post-layout function:
```rust
pub fn resolve_fields(layout: &mut PaginatedLayout, doc: &Document);
```
This function is called by the layout runner immediately after `layout_document` returns a `PaginatedLayout`. 

**Mechanism:**
1. During the walk of `layout.pages`, the resolver tracks the current `page_number` and the total page count (`pages.len()`).
2. When the resolver encounters a `PositionedGlyphRun` carrying a `FieldMetadata` marker:
   - It computes the actual display string (e.g., `page_number` for `PageNumber`).
   - It re-shapes this display string using Parley (sharing the run's font cache and character properties).
   - It calculates the width difference: `width_diff = new_run_width - placeholder_run_width`.
   - It replaces the glyphs in the `PositionedGlyphRun`.
   - It shifts all subsequent layout items on the same line (matching baseline/y-coordinate) by adding `width_diff` to their horizontal position.

This approach keeps `loki-vello` completely stateless and leaves text shaping/layout concerns inside `loki-layout`.

### 4. TOC generation algorithm
The Table of Contents is handled in two stages:

1. **Structural Update (Document Model)**:
   - When headings are added, removed, or edited, the editor model hook triggers a regeneration of the `TableOfContentsBlock::body` within the document model itself. This is done prior to running layout and results in a full layout pass.
2. **Page-Number Update (Post-Layout)**:
   - During the first-pass layout, as headings are processed, we construct a `HeadingMap: HashMap<String, usize>` mapping heading target IDs/anchors to their final page numbers.
   - In the TOC paragraphs, the page numbers are placed as `Inline::Field` of kind `FieldKind::CrossReference` pointing to the respective heading target ID.
   - The post-layout resolver looks up the target heading IDs in the `HeadingMap` and replaces the page-number field placeholders in the TOC with the resolved page numbers.

### 5. FieldMetadata / FieldPlaceholder design
We will attach a `FieldMetadata` structure to [PositionedGlyphRun](file:///d:/project/loki/loki-layout/src/items.rs#L104):

```rust
/// Metadata attached to a glyph run that originated from a dynamic field.
#[derive(Debug, Clone)]
pub struct FieldMetadata {
    /// The semantic kind of the field.
    pub kind: FieldKind,
    /// The placeholder text used during layout (for width estimation).
    pub placeholder: String,
}
```

We add this as an optional field on `PositionedGlyphRun`:
```rust
pub struct PositionedGlyphRun {
    ...
    /// Optional field metadata if this run represents a dynamic field.
    pub field: Option<FieldMetadata>,
}
```

This makes it easy for the post-layout resolver to identify and rewrite field text runs.

---

## Implementation order

1. **Add `FieldMetadata` to layout output**:
   - Update `PositionedGlyphRun` in `loki-layout` to include the `field: Option<FieldMetadata>` field.
   - Update `walk_inlines` to attach this metadata to generated glyph runs.
2. **Post-layout `PAGE` and `NUMPAGES` resolution**:
   - Implement `resolve_fields` walking `LayoutPage` content/headers/footers.
   - Implement glyph replacement and baseline shifting.
3. **Post-layout `PAGEREF` and `CROSSREF` resolution**:
   - Collect heading page numbers and resolve cross-reference targets.
4. **`DATE`/`TIME` post-layout resolution**:
   - Resolve date/time using `std::time::SystemTime`.
5. **TOC page-number resolution integration**:
   - Tie heading tracking into the layout pass and link it to the post-layout cross-reference lookup.
6. **Import updates**:
   - Verify `TITLE`, `AUTHOR`, and `FILENAME` map to `current_value` at import time (and fall back cleanly if absent).

---

## Placeholder width strategy

To minimize line-wrap breakage during post-layout substitution:
- `PageNumber` and `PageCount` fields are laid out using a fixed `"000"` (or `"00"`) placeholder. In most documents (under 100 or 1000 pages), the actual resolved text (e.g. `"7"` or `"42"`) will be narrower than the placeholder. Shrinking the width moves subsequent text leftward, which never causes wrapping overflow.
- `CrossReference` fields default to `"000"`.
- `Date` and `Time` fields use standard-width format placeholders (e.g., `"YYYY-MM-DD"` or `"HH:MM"`).

This ensures that the post-layout text width is almost always smaller than or equal to the placeholder, avoiding right-margin overflow.

---

## Out of scope
- Editing fields inline inside the editor UI (treated as plain text runs in the editor buffers).
- Custom field formulas or expressions.
- Gating field updates (field locking).
- Conditional fields (e.g., `IF` blocks).

---

## Open questions

1. **Continuous layout fields**:
   - How should `PAGE` / `NUMPAGES` resolve under `ContinuousLayout`? Since continuous layouts have no pages, they should default to a fallback value like `"1"` or the cached `current_value` snapshot.
2. **Header/Footer pagination context**:
   - Headers and footers are laid out once per page template, but their field values (e.g., `PAGE` in header/footer) vary per page. The post-layout resolver must process header and footer items for *each page* independently, using that page's specific number.
