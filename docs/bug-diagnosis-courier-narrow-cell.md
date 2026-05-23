# Bug Diagnosis: Narrow Cell + Tofu

## Bug 2: Narrow cell wrapping
### Cell width confirmed? (400 twips → Xpt)
Yes. A cell width of `w:w="400" w:type="dxa"` is correctly converted to `20.0pt`. In [table.rs](file:///d:/project/loki/loki-ooxml/src/docx/mapper/table.rs), `build_col_specs` maps the grid column widths by dividing the twips value by `20.0` (line 132):
```rust
ColWidth::Fixed(Points::new(f64::from(w) / 20.0))
```
This correctly resolves `400 twips / 20 = 20.0pt`.

### Root cause
In [flow.rs](file:///d:/project/loki/loki-layout/src/flow.rs), paragraph layout inside a table cell is computed using the cell's resolved width minus the cell's left and right margins/padding (line 970):
```rust
let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
```
`AppThere_Iris_Blueprint.docx` specifies a cell width of `20.0pt` with left/right cell margins of `7.0pt` each (`w:tcMar` left and right are `140 twips`). The resulting available layout width for paragraph text is `20.0pt - 7.0pt - 7.0pt = 6.0pt`.

At `6.0pt` layout width, laying out the string `"NOTE"` (or other narrow cell texts like `"W"`, `"H"`, `"I"`) via Parley's line breaking uses the default Unicode Line Breaking algorithm (UAX #14).
* UAX #14 does not break single words (such as `"NOTE"`) character-by-character without specific breaking rules or properties enabled.
* Parley's default style properties do not specify custom `WordBreak` or `OverflowWrap` rules.
* Consequently, the word is treated as a single line, measuring `~33.3pt` in width, which overflows the narrow cell boundary and renders as a single horizontal line extending past the cell instead of wrapping character-by-character.

### Is minimum width guard involved?
No. There is no minimum cell width guard in the layout engine or the OOXML importer. Column width resolution resolves the width to `20.0pt`, and the paragraph layout receives `6.0pt` available width.
* The table row height has a minimum guard: `row_heights[row_idx] = row_heights[row_idx].max(crate::MIN_ROW_HEIGHT);` where `MIN_ROW_HEIGHT = 0.0`.
* The background `FilledRect` is drawn correctly at the resolved cell width (`20.0pt`). The visual layout width of the blue column is correct, confirming the issue lies entirely in Parley's line breaking.

---

## Bug 3: Tofu on Windows
### Affected code points
* `✓` (U+2713 CHECK MARK) in runs using the `Calibri` font.

### Fallback font on Windows
On Windows, Parley/fontique resolves fallbacks via DirectWrite using `IDWriteFontFallback::MapCharacters`.
1. To invoke the DirectWrite fallback mapping, `fontique` queries a sample string from the script of the character being mapped using `ScriptExt::sample(&self)` in [script.rs](file:///C:/Users/kdesl/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/fontique-0.8.0/src/script.rs).
2. The `✓` (U+2713) character belongs to the `Common` (Zyyy) script.
3. In `fontique`'s `SCRIPT_SAMPLES` list in `script.rs`, there is **no sample** defined for the `Common` (Zyyy) script.
4. As a result, `key.script().sample()` returns `None`, causing `SystemFonts::fallback` in [dwrite.rs](file:///C:/Users/kdesl/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/fontique-0.8.0/src/backend/dwrite.rs) to return `None` immediately, failing to query DirectWrite for fallback families.
5. `fontique` then falls back to its hardcoded `DEFAULT_GENERIC_FAMILIES` (e.g. `Segoe UI`, `Segoe UI Emoji`), which do not include `Segoe UI Symbol` (the system font on Windows containing U+2713), resulting in a tofu block.

### Fix recommendation
1. **Script fallback sample**: Patch `fontique` to provide a fallback sample for `Common` script or fall back to a default script sample (like `Latn` / `"a"`) when mapping `Common`/`Inherited` characters.
2. **Loki fallback configuration**: In Loki's initialization code where `FontResources` is created, configure symbol fallbacks on the font collection:
   ```rust
   // Register Segoe UI Symbol/Emoji as a fallback for Common script or generally.
   collection.append_fallbacks(FallbackKey::new(Script::Common, None), vec![segoe_ui_symbol_id]);
   ```

---

## Recommended fix order
1. **Bug 2 (Narrow Cell Wrapping)**: Update `loki-layout` paragraph formatting properties to push `StyleProperty::WordBreak(WordBreak::BreakAll)` or `StyleProperty::OverflowWrap(true)` onto the Parley ranged builder when available width is narrow or as a default text wrap setting.
2. **Bug 3 (Tofu on Windows)**: Add fallback registration for `Script::Common` or register `Segoe UI Symbol` as a fallback family in `FontResources` initialization.
