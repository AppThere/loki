// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! CLI wrapper over the pinned PDF→PNG rasterizer (Spec 02 D3), used by
//! `scripts/generate-odf-goldens.sh` so golden generation goes through the
//! exact same stage as every other rasterization.
//!
//! Usage: `cargo run -p appthere-conformance --example rasterize_pdf -- <pdf> <out_dir> <stem>`

use appthere_conformance::PdfRasterizer;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(pdf), Some(out_dir), Some(stem)) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: rasterize_pdf <pdf> <out_dir> <stem>");
        std::process::exit(2);
    };
    let rasterizer = PdfRasterizer::new().expect("pdftoppm must be installed (poppler-utils)");
    let pages = rasterizer
        .rasterize(
            std::path::Path::new(&pdf),
            std::path::Path::new(&out_dir),
            &stem,
        )
        .expect("rasterization failed");
    println!("{}", rasterizer.version());
    for page in pages {
        println!("{}", page.display());
    }
}
