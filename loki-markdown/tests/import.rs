// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end Markdown import: structure lands on the right blocks, lists
//! land on the modern `StyledPara` + `list_id` representation with the
//! default styles seeded, and inline emphasis nests.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::io::DocumentImport;
use loki_doc_model::style::list_defaults::{DEFAULT_BULLET_LIST_ID, DEFAULT_NUMBERED_LIST_ID};
use loki_markdown::MarkdownImport;

const SOURCE: &str = "\
# Title

Some *emphasis* and **strength** and `code` and a [link](https://example.com).

> A quoted thought.

- top
- another
  - nested
1. first
2. second

```rust
fn main() {}
```

| A | B |
|---|---|
| 1 | 2 |

---
";

fn import(src: &str) -> loki_doc_model::document::Document {
    MarkdownImport::import(Cursor::new(src.as_bytes().to_vec()), ()).expect("import")
}

#[test]
fn structure_maps_onto_the_expected_blocks() {
    let doc = import(SOURCE);
    let blocks = &doc.sections[0].blocks;

    assert!(matches!(&blocks[0], Block::Heading(1, _, _)));
    assert!(matches!(&blocks[1], Block::Para(_)));
    // Blockquote paragraphs carry the template's Blockquote style directly.
    let Block::StyledPara(quote) = &blocks[2] else {
        panic!("expected blockquote para, got {:?}", blocks[2]);
    };
    assert_eq!(
        quote.style_id.as_ref().map(|s| s.as_str()),
        Some("Blockquote")
    );

    // Code block with its fence language.
    let code = blocks
        .iter()
        .find_map(|b| match b {
            Block::CodeBlock(attr, body) => Some((attr, body)),
            _ => None,
        })
        .expect("code block");
    assert_eq!(code.1, "fn main() {}");
    assert!(
        code.0
            .kv
            .iter()
            .any(|(k, v)| k == "language" && v == "rust")
    );

    // Table and rule survive.
    assert!(blocks.iter().any(|b| matches!(b, Block::Table(_))));
    assert!(blocks.iter().any(|b| matches!(b, Block::HorizontalRule)));
}

#[test]
fn lists_use_the_modern_representation_with_seeded_styles() {
    let doc = import(SOURCE);
    let items: Vec<(String, u8)> = doc.sections[0]
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::StyledPara(p) => {
                let pp = p.direct_para_props.as_ref()?;
                let id = pp.list_id.as_ref()?.as_str().to_string();
                Some((id, pp.list_level.unwrap_or(0)))
            }
            _ => None,
        })
        .collect();

    assert_eq!(
        items,
        vec![
            (DEFAULT_BULLET_LIST_ID.to_string(), 0),
            (DEFAULT_BULLET_LIST_ID.to_string(), 0),
            (DEFAULT_BULLET_LIST_ID.to_string(), 1), // nested
            (DEFAULT_NUMBERED_LIST_ID.to_string(), 0),
            (DEFAULT_NUMBERED_LIST_ID.to_string(), 0),
        ]
    );

    // The referenced definitions were seeded — an export straight after
    // import resolves them without any fallback.
    for id in [DEFAULT_BULLET_LIST_ID, DEFAULT_NUMBERED_LIST_ID] {
        assert!(
            doc.styles
                .list_styles
                .contains_key(&loki_doc_model::style::list_style::ListId::new(id)),
            "missing seeded list style {id}"
        );
    }
}

#[test]
fn inline_wrappers_nest_and_links_carry_targets() {
    let doc = import(SOURCE);
    let Block::Para(inlines) = &doc.sections[0].blocks[1] else {
        panic!("expected para");
    };
    assert!(inlines.iter().any(|i| matches!(i, Inline::Emph(_))));
    assert!(inlines.iter().any(|i| matches!(i, Inline::Strong(_))));
    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Code(_, c) if c == "code"))
    );
    let link = inlines
        .iter()
        .find_map(|i| match i {
            Inline::Link(_, body, target) => Some((body, target)),
            _ => None,
        })
        .expect("link");
    assert_eq!(link.1.url, "https://example.com");
    assert!(matches!(&link.0[..], [Inline::Str(s)] if s == "link"));
}
