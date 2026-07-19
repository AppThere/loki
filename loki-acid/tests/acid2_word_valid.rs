// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Guards that the committed ACID 2 fixture opens in schema-strict Microsoft
//! Word — i.e. its `WordprocessingML` parts are in the required `xsd:sequence`
//! order. The `gen_acid2_docx` generator normalises this via
//! `loki_ooxml::repair_docx`; if the fixture is regenerated without that step
//! (or hand-edited), this test fails with the specific violations.

use loki_ooxml::analyze_docx;

const ACID2: &[u8] = include_bytes!("../assets/acid2-docx.docx");

#[test]
fn committed_fixture_has_no_ordering_violations() {
    let report = analyze_docx(ACID2).expect("fixture is a valid OPC package");
    assert!(
        report.is_clean(),
        "acid2-docx.docx has {} Word-rejecting ordering violation(s) — regenerate with \
         `cargo run -p loki-acid --example gen_acid2_docx`:\n{}",
        report.findings.len(),
        report
            .findings
            .iter()
            .map(|f| format!("  [{}] <{}>: {}", f.part, f.container, f.detail))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
