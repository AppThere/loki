# ADR-0006: CRDT-Based Document Mutation Model

**Status:** Proposed  
**Date:** 2026-04-25  
**Deciders:** AppThere engineering

---

## Context

Loki's current pipeline is entirely read-only: `loki-ooxml` / `loki-odf` parse a
file into a `loki-doc-model::Document`, which `loki-layout` consumes to produce a
paginated render tree. There is no mutation API anywhere in the codebase.

The next major milestone is rich text editing. Rather than bolt on a simple
mutation log later and rearchitect for collaboration, the design adopts a CRDT
from the start. CRDTs give:

- **Undo/redo** — via an `UndoManager` that understands operation semantics.
- **Collaboration** — peers can apply each other's operation streams and
  converge to the same state without a central arbiter.
- **Conflict resolution** — guaranteed by construction; no "last writer wins"
  data loss.

The chosen CRDT library is **Loro** (`loro` crate, version 1.11.1, released
2025-04-21). Loro is a production-grade Rust CRDT library used by several
collaborative editors. It implements the Peritext algorithm for rich text and
Fugue for sequences, which are the two algorithms most relevant to document
editing.

---

## Step 1 audit: Loro's current integration in loki-doc-model

| Question | Finding |
|----------|---------|
| Loro version declared? | **None** — `loro` does not appear in any `Cargo.toml` in the workspace |
| Loro types used in structs? | No |
| Existing CRDT integration code? | No |
| Loro features enabled? | N/A |

`loki-doc-model` is also entirely immutable by design: there are no mutation
methods. The only "write" operations are constructor helpers (`Document::new()`,
`Cell::simple()`, etc.) and style-resolution methods
(`StyleCatalog::resolve_para`). This makes the separation between the editing
model (Loro) and the read-only snapshot model (Document) natural.

---

## Step 2: Loro 1.11.1 API summary

### Document container

```rust
// Creation
let doc = LoroDoc::new();

// Save — multiple export modes
let bytes: Vec<u8> = doc.export(ExportMode::Snapshot)?;
let bytes = doc.export(ExportMode::Updates { from: &version_vector })?;
let bytes = doc.export(ExportMode::ShallowSnapshot { target_frontiers: &f })?;

// Load
doc.import(&bytes)?;                         // merge updates or snapshot
let doc = LoroDoc::from_snapshot(&bytes)?;   // construct fresh from snapshot

// JSON variant
let json = doc.export_json_updates(...);
doc.import_json_updates(json)?;
```

Export modes: `Snapshot` (full history + state), `Updates` (delta since a
version vector), `ShallowSnapshot` (current state + partial history, efficient
for large docs), `StateOnly`, `SnapshotAt`. The primary wire format is **binary**;
a JSON mode exists for debugging.

### Rich text

`LoroText` is a dedicated rich text container using the **Peritext** mark
algorithm. Marks are stored as a **separate annotation layer** (not inline in
the character sequence).

```rust
// Get or create a LoroText from a LoroDoc
let text: LoroText = doc.get_text("content");

// Insert / delete
text.insert(pos, "hello")?;
text.delete(pos, len)?;

// Apply a character-level mark
text.mark(start..end, "bold", true)?;
text.mark(start..end, "color", "#FF0000")?;
text.mark(start..end, "underline", "single")?;

// Unmark
text.unmark(start..end, "bold")?;

// Read as Quill Delta (spans with mark maps)
let delta: Vec<TextDelta> = text.to_delta();

// Configure mark expand behavior at boundary insertions
doc.config_text_style("bold", ExpandType::After)?;

// UTF-8 / UTF-16 variants also available
text.mark_utf8(byte_range, "italic", true)?;
```

Mark values accept `impl Into<LoroValue>`:
- Primitives: `Bool`, `I64`, `Double`, `String`
- Structured: `LoroValue::Map` (nested key-value), `List`, `Binary`

Structured marks (e.g. colour as `{r, g, b}`) are therefore supported, though
Loki will use string encoding (hex or named) to keep marks JSON-serialisable.

### Block-level containers

```rust
// LoroList — ordered sequence (position-indexed)
let blocks: LoroList = doc.get_list("blocks");
let para: LoroMap = blocks.insert_container(idx, LoroMap::new())?;

// LoroMovableList — like LoroList but supports O(log n) move without
// delete+insert, preserving identity through concurrent moves
let blocks: LoroMovableList = doc.get_movable_list("blocks");

// LoroMap — key-value container; values can be any LoroValue or sub-container
let para_map: LoroMap = doc.get_map("para_0");
para_map.insert("type", "para")?;
para_map.insert_container("content", LoroText::new())?;

// LoroTree — hierarchical, each node has a metadata LoroMap, supports
// fractional-index ordering and concurrent move
let tree: LoroTree = doc.get_tree("doc_tree");
let root = tree.create(TreeParent::Root, 0)?;
let child = tree.create(&root, 0)?;
let meta: LoroMap = tree.get_meta(&root)?;
```

### Operations and history

Operations are **recorded implicitly** — any mutation (`insert`, `mark`, etc.)
is appended to the document's operation log automatically. There is no explicit
transaction API.

```rust
// Undo / redo — via UndoManager
let mut undo_manager = UndoManager::new(&doc);
undo_manager.undo(&doc)?;
undo_manager.redo(&doc)?;

// Time travel (read-only checkout for history inspection)
let past = doc.state_frontiers();
doc.checkout(&past)?;
doc.checkout_to_latest();

// Permanent revert to a historical version
doc.revert_to(&frontiers)?;
```

### Collaboration wire format

```rust
// Peer A — export only the operations peer B hasn't seen yet
let b_vv: VersionVector = receive_from_b();
let update = doc_a.export(ExportMode::Updates { from: &b_vv })?;

// Peer B — merge A's updates; convergence is automatic
doc_b.import(&update)?;

// Subscribe to locally-generated updates for network dispatch
doc.subscribe_local_update(|bytes| network.send(bytes));
```

The protocol uses version vectors (`VersionVector`) and frontier sets
(`Frontiers`) to compute missing operations. Peers can also bootstrap
from a snapshot when connecting for the first time.

### Stable cursor positions

```rust
let cursor: Cursor = text.get_cursor(pos, Side::Right);
// cursor.id anchors to the character element ID (survives concurrent inserts)

// Encode for storage or network
let bytes = cursor.encode();
let cursor = Cursor::decode(&bytes)?;

// Resolve current position after merging remote ops
let resolved: Option<usize> = text.get_cursor_pos(&cursor)?;
```

`Cursor` tracks a logical position by element identity (Fugue ID + side), not
by integer offset. Concurrent inserts before the cursor move the integer offset
without moving the logical anchor.

---

## Step 3: Prior art

No Rust-native rich text editors built on Loro were found in the registry.
Reference implementations found:

- **[`loro-dev/crdt-richtext`](https://github.com/loro-dev/crdt-richtext)** —
  Rust implementation of Peritext + Fugue that became the core of `loro`'s
  `LoroText`. Useful for understanding mark semantics.
- **Blocky Editor** ([`blocky-editor.dev/loro`](https://blocky-editor.dev/loro)) —
  TypeScript/WASM editor showing the canonical block-list architecture:
  root `LoroList` of blocks, each block a `LoroMap` with a `LoroText`.
- Loro's own tutorial pages for `text` and `list` illustrate the paragraph
  block pattern.

The Blocky Editor pattern (Option B below) is confirmed as the library's
intended usage for document editors.

---

## Decisions

### Decision 1: Loro container structure — Option B (LoroMovableList of block containers)

**Rejected: Option A — flat LoroText with block marks.**
`loki-doc-model`'s block type space is too rich: tables, figures, multi-section
layouts, and nested lists cannot be cleanly represented as marks on a flat text
sequence. Cross-block cursor selection would require complex range logic.

**Rejected: Option C — LoroTree for the full hierarchy.**
LoroTree is designed for "directories, outlines, or other movable hierarchical
data". It would handle deep nesting well but adds API complexity for the common
case (paragraphs and headings). Tables already have their own two-dimensional
structure that a flat LoroTree doesn't simplify.

**Chosen: Option B — LoroMovableList of block containers.**

The document maps to Loro as follows:

```
LoroDoc
├── "meta"    → LoroMap  (title, author, created, modified, …)
├── "styles"  → LoroMap  (paragraph_styles, character_styles, …)
└── "sections" → LoroList<LoroMap>
      └── LoroMap  (one per Section)
            ├── "layout" → LoroMap  (page_size, margins, orientation, columns)
            ├── "header" / "footer" / … → LoroMap  (kind, content)
            └── "blocks" → LoroMovableList<LoroMap>
                  └── LoroMap  (one per Block)
                        ├── "type"         → string  ("para" | "heading" | "table" | …)
                        ├── "style_id"     → string | null
                        ├── "para_props"   → LoroMap  (alignment, indent, …)
                        ├── "char_props"   → LoroMap  (direct run-level props on para)
                        └── "content"      → LoroText  (inline content with marks)
```

Tables use a nested `LoroList` of row `LoroList`s of cell `LoroMap`s, each with
their own `"content"` `LoroText`. This mirrors the existing `Table → Row → Cell`
hierarchy in `loki-doc-model`.

**Rationale:**
- Block reordering (drag-and-drop, cut/paste) maps to `LoroMovableList::mov()`,
  which preserves element identity across concurrent moves.
- Each paragraph's inline content is an isolated `LoroText`, so Peritext mark
  semantics are well-defined within a block.
- The block type hierarchy (`loki-doc-model::Block` enum variants) maps cleanly
  to the `"type"` discriminant in each block `LoroMap`.
- Cross-block selection (select across paragraph boundaries) is handled at the
  editing layer by coordinating mark operations across multiple `LoroText`
  containers — complex but tractable, and only needed for the editor, not layout.

### Decision 2: LoroDoc / Document relationship — Option Y (separate, Document is derived)

**Rejected: Option X — LoroDoc IS the document state.**
This would couple the layout engine to Loro types, require all rendering code
to speak Loro's API, and force read-only file viewing to pay Loro's initialization
cost. The existing `loki-layout` crate reads from `loki-doc-model::Document`
and is feature-complete; changing its input type would be a large regression risk.

**Chosen: Option Y — LoroDoc is the authoritative editing state;
`loki-doc-model::Document` is a derived read-only snapshot.**

```
            ┌──────────────────────────────────────────┐
            │           Editing session                │
            │                                          │
  User      │   LoroDoc ──export──▶ Document ──▶ layout│
  input ───▶│    │  ▲                (snapshot)        │
            │    │  │ import                           │
            │   network/peer                           │
            └──────────────────────────────────────────┘
```

Mutation flow:
1. User action → mutate `LoroDoc` (insert text, mark, move block, etc.)
2. Loro fires a change callback.
3. The editing layer converts the affected subtree of Loro containers to the
   corresponding `loki-doc-model` types, producing a new `Document` snapshot
   (or invalidating a cached one).
4. The layout engine re-runs on the new snapshot.

For read-only viewing (opening a file without editing):
- `loki-ooxml` / `loki-odf` populate a `Document` directly as today.
- No `LoroDoc` is created. Zero CRDT overhead.

For collaboration:
- Remote peer ops arrive via `doc.import(bytes)`.
- The change callback fires, invalidating the snapshot and triggering re-layout.

### Decision 3: Mark schema for rich text

Character-level `CharProps` fields map to `LoroText` marks. Paragraph-level
`ParaProps` fields are stored as keys on the block `LoroMap`, not as marks.

#### Character marks (on `LoroText`)

| Mark key | Value type | `CharProps` field | Notes |
|----------|-----------|-------------------|-------|
| `bold` | `bool` | `bold` | |
| `italic` | `bool` | `italic` | |
| `underline` | `string` | `underline: UnderlineStyle` | `"single"` `"double"` `"dotted"` `"dash"` `"wave"` `"thick"` |
| `strikethrough` | `string` | `strikethrough: StrikethroughStyle` | `"single"` `"double"` |
| `color` | `string` | `color: DocumentColor` | hex `"#RRGGBB"` or `"auto"` |
| `background_color` | `string` | `background_color: DocumentColor` | hex or `"auto"` |
| `highlight_color` | `string` | `highlight_color: HighlightColor` | named constant e.g. `"yellow"` |
| `font_name` | `string` | `font_name` | |
| `font_name_complex` | `string` | `font_name_complex` | |
| `font_name_east_asian` | `string` | `font_name_east_asian` | |
| `font_size_pt` | `f64` | `font_size: Points` | |
| `font_size_complex_pt` | `f64` | `font_size_complex: Points` | |
| `vertical_align` | `string` | `vertical_align: VerticalAlign` | `"superscript"` `"subscript"` `"baseline"` |
| `outline` | `bool` | `outline` | |
| `shadow` | `bool` | `shadow` | |
| `small_caps` | `bool` | `small_caps` | |
| `all_caps` | `bool` | `all_caps` | |
| `letter_spacing_pt` | `f64` | `letter_spacing: Points` | |
| `word_spacing_pt` | `f64` | `word_spacing: Points` | |
| `kerning` | `bool` | `kerning` | |
| `scale` | `f64` | `scale: f32` | stored as f64, cast on read |
| `language` | `string` | `language: LanguageTag` | BCP 47 tag |
| `language_complex` | `string` | `language_complex` | |
| `language_east_asian` | `string` | `language_east_asian` | |
| `hyperlink` | `string` \| `null` | `hyperlink` | null = no link |
| `style_id` | `string` \| `null` | `StyledRun.style_id` | character style reference |

All `Option<bool>` marks use Loro's `unmark` to remove (rather than storing
`false`), preserving the "unset" / "inherited" distinction.

#### Block-level properties (LoroMap keys, not marks)

`ParaProps` and paragraph-level `CharProps` are stored as flat string/number
values in the block `LoroMap` under a `"para_props"` sub-map and a
`"direct_char_props"` sub-map respectively. The same key names from the mark
table apply for `direct_char_props`; `para_props` uses:

| Key | Value type | `ParaProps` field |
|-----|-----------|-------------------|
| `alignment` | `string` | `"left"` `"right"` `"center"` `"justify"` `"distribute"` |
| `indent_start_pt` | `f64` | `indent_start` |
| `indent_end_pt` | `f64` | `indent_end` |
| `indent_first_line_pt` | `f64` | `indent_first_line` |
| `indent_hanging_pt` | `f64` | `indent_hanging` |
| `space_before` | `string` | `"exact:Npt"` or `"pct:N"` |
| `space_after` | `string` | same encoding |
| `line_height` | `string` | `"exact:Npt"` `"atleast:Npt"` `"multiple:N"` |
| `keep_together` | `bool` | |
| `keep_with_next` | `bool` | |
| `page_break_before` | `bool` | |
| `list_id` | `string` \| `null` | |
| `list_level` | `i64` | |
| `outline_level` | `i64` | |
| `bidi` | `bool` | |
| `border_top` / `_bottom` / `_left` / `_right` / `_between` | `string` | serialised `Border` struct |
| `tab_stops` | `string` | JSON array of `TabStop` records |
| `background_color` | `string` | hex |

### Decision 4: Block representation details

```
Block::StyledPara / Block::Para
  LoroMap {
    "type":            "para",
    "style_id":        string | null,
    "para_props":      LoroMap { … ParaProps fields … },
    "direct_char_props": LoroMap { … CharProps fields … },
    "content":         LoroText  ← inline text + character marks
  }

Block::Heading(level, …)
  LoroMap {
    "type":    "heading",
    "level":   i64,
    "style_id": string | null,
    "content": LoroText
  }

Block::Table
  LoroMap {
    "type":    "table",
    "props":   LoroMap { … TableProps … },
    "col_specs": LoroList<LoroMap>,  ← { alignment, width }
    "head":    LoroList<LoroMap>,    ← rows
    "bodies":  LoroList<LoroMap>,    ← { head_rows, body_rows }
    "foot":    LoroList<LoroMap>     ← rows
  }
  Row → LoroList<LoroMap>  (cells)
  Cell → LoroMap {
    "row_span", "col_span", "alignment",
    "props":    LoroMap { … CellProps … },
    "blocks":   LoroMovableList<LoroMap>  ← recursive block list
  }

Block::OrderedList / Block::BulletList
  LoroMap {
    "type":   "ordered_list" | "bullet_list",
    "attrs":  LoroMap { start_number, style, delimiter },
    "items":  LoroList<LoroMovableList<LoroMap>>  ← list of block lists
  }

Block::BlockQuote / Block::Div / Block::Figure
  LoroMap {
    "type":   "blockquote" | "div" | "figure",
    "blocks": LoroMovableList<LoroMap>
  }

Block::HorizontalRule / Block::CodeBlock / Block::RawBlock / Block::Math
  LoroMap { "type": …, "content": LoroText | string }
```

### Decision 5: Undo/redo mechanism

```rust
let undo_manager = UndoManager::new(&doc);
undo_manager.undo(&doc)?;   // collapse the last local operation group
undo_manager.redo(&doc)?;
```

`UndoManager` is CRDT-aware: it does not blindly replay inverse operations but
computes a transform that is safe under concurrent edits. Remote operations
interleaved with local history do not corrupt the undo stack.

The editing layer wraps logically-related mutations (e.g. "user typed 'abc'")
into a single undo group by calling `doc.commit()` between groups.

Time-travel inspection (e.g. "show me this document as of commit X"):

```rust
let frontiers: Frontiers = doc.oplog_frontiers();
doc.checkout(&frontiers)?;   // read-only view at that point
doc.checkout_to_latest();    // return to current
```

### Decision 6: Collaboration wire format

```
Peer A                           Peer B
  │                                │
  │── subscribe_local_update ──┐   │
  │                            │   │
  │  user edit                 │   │  user edit
  │       │                    │   │       │
  │   LoroDoc mutated          │   │   LoroDoc mutated
  │       │                    │   │       │
  │       └── callback ──── bytes ──▶ doc_b.import(bytes)
  │                                │       │
  │   doc_a.import(bytes) ◀── bytes ── callback
```

Bootstrap for a new peer:
```rust
// Host exports a shallow snapshot (state + recent history)
let snapshot = doc.export(ExportMode::ShallowSnapshot { target_frontiers: &f })?;
// New peer loads it
let doc_new = LoroDoc::from_snapshot(&snapshot)?;
// Then receives incremental updates via the normal path
```

Version vectors are exchanged on handshake so each peer exports only the delta
the other peer is missing.

### Decision 7: Cursor stability

`Cursor` objects anchor to Fugue element IDs, not integer positions:

```rust
// Before applying remote ops — save cursor
let cursor = text.get_cursor(caret_pos, Side::Right);

// Apply remote ops
doc.import(&remote_bytes)?;

// Resolve new integer position
let new_pos = text.get_cursor_pos(&cursor)?.pos;
```

The editing layer holds a `Cursor` (not a `usize`) for every live caret and
selection endpoint. After any `import()` or local mutation, it resolves all
cursors to integer positions before redrawing.

### Decision 8: Import/export bridge

**Populating LoroDoc from file (loki-ooxml / loki-odf → LoroDoc):**

An `IntoLoro` trait (in a new `loki-loro` crate, see implementation order) will
walk a `loki-doc-model::Document` and produce a `LoroDoc`. This is a one-time
conversion at file-open time.

```
loki-ooxml/loki-odf → Document → IntoLoro::convert → LoroDoc
```

Initial population does not need per-character CRDT IDs for the existing content
since there is no concurrent peer yet; a single bulk import suffices.

**Deriving Document from LoroDoc (LoroDoc → Document):**

A `FromLoro` trait reads the Loro container tree and constructs a
`loki-doc-model::Document`. The editing layer caches the last snapshot and the
change set from the Loro callback to do incremental updates:

```
LoroDoc change event → FromLoro::convert_changed_blocks → Document (updated)
                                                              │
                                                         loki-layout
```

For the first milestone a full re-derive on every change is acceptable; partial
invalidation is an optimization for a later milestone.

---

## Implementation order

1. **`loki-loro` crate scaffold** — add `loro = "1.11.1"` as a dependency, define
   the `LoroStore` struct wrapping `LoroDoc`, expose `new()` / `import()` /
   `export()` / `subscribe_changes()`.

2. **`IntoLoro` — Document → LoroDoc** — implement conversion walking the
   `Document` tree into the Loro container schema defined above. Covers all block
   types, `CharProps` marks, `ParaProps` map entries, and the `StyleCatalog`.

3. **`FromLoro` — LoroDoc → Document** — inverse conversion. Write round-trip
   tests: `Document → LoroDoc → Document` must be lossless for all
   `loki-doc-model` types.

4. **`UndoManager` wiring** — integrate with the editing session; define undo
   group boundaries (commit after each discrete user action).

5. **Character-level mutation API** — expose `insert_text`, `delete_text`, and
   `mark_range` on `LoroStore`, with `Cursor` input/output for stable positions.

6. **Block-level mutation API** — `insert_block`, `delete_block`, `move_block`,
   and `split_paragraph` (splits one block's `LoroText` at a cursor position).

7. **Collaboration wire layer** — wire `subscribe_local_update` to the network
   transport stub; implement the handshake (version vector exchange + snapshot
   bootstrap for new peers).

8. **Incremental `FromLoro` invalidation** — instead of full re-derive, use the
   Loro change event's block IDs to re-derive only affected blocks.

---

## Out of scope

- IME (Input Method Editor) input handling
- AccessKit / accessibility integration
- Real-time network transport implementation
- Conflict-resolution UI (concurrent edit indicators)
- Spell-check / grammar integration
- Comment / tracked-change CRDT semantics (addressed in a separate ADR)

---

## Open questions

1. **Style CRDT semantics** — `StyleCatalog` is shared document state. Should
   the style catalog itself live in `LoroDoc` (so two peers can collaboratively
   define styles), or remain a per-peer local structure loaded from the file and
   immutable during a session? The common case (OOXML/ODF files) treats styles as
   static, so the question only matters for true multi-user document authoring.

2. **Section/layout CRDT semantics** — `PageLayout` (margins, page size) is
   rarely edited concurrently, but is it meaningful to CRDT it? A simpler model
   stores section layout as a `LoroMap` but resolves conflicts with last-writer-wins
   on individual keys. Open question: should there be a merge strategy for
   `margins.left` vs `margins.right` when two peers both edit?

3. **Table merge conflicts** — concurrent row insertion into the same table is
   handled by `LoroList` naturally, but concurrent cell span changes (`row_span`,
   `col_span`) could produce structurally invalid tables. A validation pass after
   every `import()` may be required.

4. **Loro version compatibility** — Loro's binary format is explicitly versioned
   but breaking changes between minor releases are possible while the library is
   pre-1.0 (despite the current 1.x version label, the project notes ongoing
   format evolution). Pinning `loro = "=1.11.1"` and treating upgrades as a
   migration event is recommended until the format is declared stable.

5. **`ExtensionBag` in Loro** — `loki-doc-model` uses `ExtensionBag` to carry
   format-specific data (OOXML/ODF extensions) through the pipeline. The mapping
   from `ExtensionBag` into Loro is not yet defined; one option is to serialize
   it to `Binary` in the block `LoroMap`, but this makes the data opaque to
   collaborative transforms.
