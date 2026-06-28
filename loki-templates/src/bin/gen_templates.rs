// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regenerates the bundled `.dotx` template assets from the builders.
//!
//! Run with `cargo run -p loki-templates --bin gen_templates`. Writes one
//! `assets/<id>.dotx` per entry in `loki_templates::TEMPLATES`.

// Offline codegen tool, not library runtime: a panic aborts the regeneration
// run (and is the desired failure mode), so unwrap/expect are fine here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Cursor;
use std::path::Path;

use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxTemplateExport;

fn main() {
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    std::fs::create_dir_all(&out_dir).expect("create assets dir");

    for t in loki_templates::TEMPLATES {
        let doc = loki_templates::build_document(t.id).expect("known template id");
        let mut buf = Cursor::new(Vec::new());
        DocxTemplateExport::export(&doc, &mut buf, ()).expect("export .dotx");
        let path = out_dir.join(format!("{}.dotx", t.id));
        std::fs::write(&path, buf.into_inner()).expect("write asset");
        println!("wrote {}", path.display());
    }
}
