// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration smoke tests: rich ODT fixture → import → assert document shape.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_odf::odt::import::{OdtImporter, OdtImportOptions};

/// Import a rich ODT fixture and validate the high-level document shape.
///
/// Checks:
/// 1. Document has at least one block.
/// 2. At least one block is a `Block::Heading`.
/// 3. At least one block is a `Block::BulletList`.
#[test]
fn import_rich_odt_smoke() {
    let content = helpers::rich_fixture_content_xml("1.2");
    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("rich ODT fixture should import without error");

    let doc = &result.document;

    // ── 1. Non-empty content ────────────────────────────────────────────────
    let all_blocks: Vec<&Block> =
        doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    assert!(!all_blocks.is_empty(), "document must contain at least one block");

    // ── 2. At least one Heading ─────────────────────────────────────────────
    let has_heading = all_blocks.iter().any(|b| matches!(b, Block::Heading(..)));
    assert!(has_heading, "at least one Block::Heading must be present");

    // ── 3. At least one BulletList ──────────────────────────────────────────
    let has_bullet_list = all_blocks.iter().any(|b| matches!(b, Block::BulletList(..)));
    assert!(has_bullet_list, "at least one Block::BulletList must be present");
}
