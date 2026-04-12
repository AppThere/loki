# ADR-0003: Flatten ODF list structure by injecting list context onto paragraphs

**Status:** Accepted  
**Date:** 2024-10-01  
**Deciders:** AppThere engineering

---

## Context

In ODF, list content is encoded as a tree:

```xml
<text:list text:style-name="L1">
  <text:list-item>
    <text:p text:style-name="P1">Item one</text:p>
    <text:list>                          <!-- nested list -->
      <text:list-item>
        <text:p>Sub-item</text:p>
      </text:list-item>
    </text:list>
  </text:list-item>
</text:list>
```

The `text:list` and `text:list-item` elements are structural wrappers; the
actual formatted content is always in `text:p` (or `text:h`) elements.

The format-neutral model (`loki_doc_model`) uses `Block::BulletList` and
`Block::OrderedList`, each containing `Vec<Vec<Block>>` — a list of items,
each item being a sequence of blocks.

## Decision

The XML reader (`reader/document.rs`) reads `text:list` recursively and
reconstructs `OdfList` / `OdfListItem` / `OdfListItemChild` structures
from the XML tree. It also injects an `OdfListContext` onto every `OdfParagraph`
that appears inside a list item, recording the style name and nesting depth.

The mapper (`mapper/document.rs`) then converts `OdfBodyChild::List(OdfList)`
to either `Block::BulletList` or `Block::OrderedList` by inspecting the first
level of the named list style in the catalog.

## Rationale

Preserving the structural list information in the intermediate model (rather
than flattening it at read time) allows:

1. Accurate nesting depth — the mapper sees the complete `OdfList` tree and
   can produce correctly nested `Block::OrderedList`/`Block::BulletList`.
2. Style lookup — the `OdfList::style_name` is preserved so the mapper can
   look up the resolved list style in the catalog to determine whether the
   list is ordered or unordered.
3. `continue-list` and `continue-numbering` attributes are available on
   `OdfList` for future use.

The `OdfListContext` injected onto paragraphs is a denormalised cache that
would be useful for exporters needing to re-wrap paragraphs in `text:list`
elements without re-walking the tree.

## Consequences

- `OdfListItemChild::Paragraph` and `OdfListItemChild::Heading` hold the same
  `OdfParagraph` type as body-level content, keeping the model uniform.
- The mapper calls `map_list_item` recursively for nested lists, which
  naturally produces nested `Block::BulletList`/`Block::OrderedList` items.
