// SPDX-License-Identifier: Apache-2.0

//! End-to-end CLI tests: run the real binary on real files.

use std::io::Cursor;
use std::process::Command;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_loki-headless"))
}

fn write_sample_docx(path: &std::path::Path) {
    let mut doc = Document::new();
    if let Some(section) = doc.sections.first_mut() {
        section.blocks.push(Block::Para(vec![Inline::Str(
            "Printed from the office NUC by the print room.".into(),
        )]));
    }
    let mut cursor = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut cursor, ()).expect("fixture");
    std::fs::write(path, cursor.into_inner()).expect("write fixture");
}

#[test]
fn convert_docx_to_pdf_via_cli() {
    let dir = std::env::temp_dir().join(format!("loki-headless-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("report.docx");
    let output = dir.join("report.pdf");
    write_sample_docx(&input);

    let status = bin()
        .args(["convert", "--in"])
        .arg(&input)
        .arg("--out")
        .arg(&output)
        .args(["--profile", "pdf-x4"])
        .status()
        .expect("run CLI");
    assert!(status.success());
    let pdf = std::fs::read(&output).expect("output written");
    assert!(pdf.starts_with(b"%PDF-1."));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unsupported_conversion_fails_with_typed_message() {
    let dir = std::env::temp_dir().join(format!("loki-headless-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("deck.pptx");
    std::fs::write(&input, b"not really a pptx").unwrap();

    let output = bin()
        .args(["convert", "--in"])
        .arg(&input)
        .arg("--out")
        .arg(dir.join("deck.pdf"))
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not supported"),
        "expected a ConversionUnsupported message, got: {stderr}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn formats_lists_the_matrix() {
    let output = bin().arg("formats").output().expect("run CLI");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("docx -> pdf"));
    assert!(stdout.contains("ods -> xlsx"));
    assert!(!stdout.contains("pptx"), "gated formats must not be listed");
}
