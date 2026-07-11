// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Picture-bullet image resolution for DOCX import (feature 5.4).
//!
//! `w:numPicBullet` references its image through `word/numbering.xml.rels` — a
//! *separate* relationship set from the document's — so it is resolved here,
//! after `word/numbering.xml` is parsed, and baked onto the numbering model as
//! a `data:` URI. Kept out of `import.rs` to hold that file's line ceiling.

use loki_opc::{Package, RelationshipSet};

use crate::constants::REL_NUMBERING;
use crate::docx::model::numbering::DocxNumbering;

/// Resolve each `w:numPicBullet`'s image relationship to a `data:` URI on the
/// numbering model, so picture bullets render. No-op when images are not
/// embedded, there is no numbering / picture bullets, or the numbering part has
/// no relationships.
pub(super) fn resolve(
    package: &Package,
    doc_rels: Option<&RelationshipSet>,
    doc_part_name: &str,
    numbering: Option<&mut DocxNumbering>,
    embed_images: bool,
) {
    use base64::Engine as _;
    let Some(numbering) = numbering else { return };
    if !embed_images || numbering.pic_bullets.is_empty() {
        return;
    }
    let Some(doc_rels) = doc_rels else { return };
    let Some(rel) = super::rels_by_type(doc_rels, REL_NUMBERING).next() else {
        return;
    };
    let Ok(num_part) = super::resolve_part_name(doc_part_name, &rel.target) else {
        return;
    };
    let Some(num_rels) = package.part_relationships(&num_part) else {
        return;
    };
    for pb in &mut numbering.pic_bullets {
        let Some(img_rel) = num_rels.iter().find(|r| r.id == pb.rel_id) else {
            continue;
        };
        let Ok(img_name) = super::resolve_part_name(num_part.as_str(), &img_rel.target) else {
            continue;
        };
        if let Some(part) = package.part(&img_name) {
            pb.src = Some(format!(
                "data:{};base64,{}",
                part.media_type,
                base64::engine::general_purpose::STANDARD.encode(&part.bytes),
            ));
        }
    }
}
