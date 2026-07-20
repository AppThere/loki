// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Guards that the committed ACID 2 fixture opens in schema-strict Microsoft
//! Word. [`analyze_docx`] checks both Word-vs-tolerant asymmetries: the
//! `WordprocessingML` parts must be in the required `xsd:sequence` order, and
//! every `mc:Ignorable` prefix must resolve to a declared namespace (an
//! undeclared `w14` in `styles.xml` once made Word reject this file while Loki
//! opened it fine). The `gen_acid2_docx` generator normalises ordering via
//! `loki_ooxml::repair_docx`; if the fixture is regenerated without that step
//! (or hand-edited into either defect), this test fails with the specifics.

use std::io::Cursor;

use loki_ooxml::analyze_docx;
use loki_opc::Package;
use loki_opc::part::PartName;

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

/// A separator-reference block in `settings.xml` (`<w:footnotePr>` /
/// `<w:endnotePr>`) points at the special separator notes that live in the
/// corresponding part. If that part is absent, Word reports an error in
/// "Footnotes"/"Endnotes" on open — the exact failure this fixture hit with a
/// spurious `<w:endnotePr>` and no `endnotes.xml`. Loki opens it regardless, so
/// only a package-level cross-part check catches it. (This is an OPC-level
/// invariant `analyze_docx` does not yet model — see fidelity-status §12.)
#[test]
fn note_separator_refs_have_a_backing_part() {
    let pkg = Package::open(Cursor::new(ACID2)).expect("valid OPC package");
    let settings = pkg
        .part(&PartName::new("/word/settings.xml").expect("part name"))
        .expect("settings.xml present");
    let settings = String::from_utf8_lossy(&settings.bytes);

    for (block, part) in [
        ("<w:footnotePr>", "/word/footnotes.xml"),
        ("<w:endnotePr>", "/word/endnotes.xml"),
    ] {
        if settings.contains(block) {
            assert!(
                pkg.part(&PartName::new(part).expect("part name")).is_some(),
                "settings.xml declares {block} but the fixture has no {part} to back its \
                 separator notes — Word will report an error in that notes stream",
            );
        }
    }
}
