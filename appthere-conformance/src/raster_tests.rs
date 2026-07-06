// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;

/// A minimal but complete one-page PDF (blank A4-ish page) for exercising the
/// rasterizer without depending on an export crate.
const TINY_PDF: &[u8] = b"%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >> endobj
xref
0 4
0000000000 65535 f
0000000009 00000 n
0000000058 00000 n
0000000115 00000 n
trailer << /Size 4 /Root 1 0 R >>
startxref
187
%%EOF
";

#[test]
fn version_is_captured() {
    let r = PdfRasterizer::new().expect("pdftoppm must be installed (poppler-utils)");
    assert!(
        r.version().contains("pdftoppm"),
        "version banner should identify the backend: {:?}",
        r.version()
    );
}

#[test]
fn rasterizes_a_page_deterministically() {
    let r = PdfRasterizer::new().expect("pdftoppm must be installed");
    let dir = tempfile::tempdir().expect("tempdir");
    let pdf = dir.path().join("tiny.pdf");
    std::fs::write(&pdf, TINY_PDF).expect("write pdf");

    let pages_a = r
        .rasterize(&pdf, &dir.path().join("a"), "page")
        .expect("rasterize A");
    assert_eq!(pages_a.len(), 1, "one page expected");

    // D3's whole point: the stage is deterministic — two runs, identical bytes.
    let pages_b = r
        .rasterize(&pdf, &dir.path().join("b"), "page")
        .expect("rasterize B");
    let a = std::fs::read(&pages_a[0]).expect("read A");
    let b = std::fs::read(&pages_b[0]).expect("read B");
    assert!(!a.is_empty());
    assert_eq!(a, b, "pinned rasterization must be byte-deterministic");
}

#[test]
fn missing_pages_is_an_error_not_empty_ok() {
    let r = PdfRasterizer::new().expect("pdftoppm must be installed");
    let dir = tempfile::tempdir().expect("tempdir");
    let pdf = dir.path().join("broken.pdf");
    std::fs::write(&pdf, b"not a pdf").expect("write");
    assert!(
        r.rasterize(&pdf, dir.path(), "page").is_err(),
        "a broken PDF must error, not return zero pages"
    );
}
