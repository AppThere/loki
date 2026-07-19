// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generates `loki-acid/assets/acid2-docx.docx`, the **ACID 2** DOCX fixture,
//! and mirrors it into `appthere-conformance/fixtures/docx/acid2-docx.docx`.
//!
//! Run with `cargo run -p loki-acid --example gen_acid2_docx`.
//!
//! # Provenance — read before trusting this fixture
//!
//! Unlike `acid_docx.docx` (authored in real Microsoft Word to test *import*
//! fidelity against genuine Office output), ACID 2 is **hand-authored OOXML**:
//! the WordprocessingML parts live as reviewable XML under `assets/acid2/` and
//! this generator packages them into a Word-valid OPC container via `loki-opc`.
//! That is deliberate — it lets the fixture exercise constructs Loki cannot yet
//! *emit* (text boxes, page borders, tracked changes, pattern shading, …),
//! which a round-trip-through-Loki fixture never could. Parity is still judged
//! against Microsoft Word's render of *this same file*: open it in Word, print
//! to PDF, and run `scripts/generate-office-goldens.sh` (see the golden dir's
//! `PENDING.txt`). ACID 2 combines features into realistic archetypes — a
//! business report, a résumé, a newsletter, an invoice/contract — plus an
//! isolated feature-matrix appendix, because interaction bugs are what break
//! real documents.

use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

// ── OPC relationship-type URIs (ECMA-376 §17 / OPC) ──────────────────────────
const NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
fn rel(kind: &str) -> String {
    format!("{NS}/{kind}")
}

// ── WordprocessingML media types ─────────────────────────────────────────────
const MT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const MT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const MT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const MT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const MT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
const MT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const MT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
const MT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const MT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";

/// One document-part relationship to declare on `word/document.xml`.
struct DocRel {
    r_id: &'static str,
    kind: &'static str,
    /// Relationship target (relative to `word/`).
    target: &'static str,
    /// The part's absolute OPC name, or `None` for external targets.
    part: Option<&'static str>,
    media_type: &'static str,
    external: bool,
}

/// The document parts, in the order their `rId`s appear in `document.xml`.
const DOC_RELS: &[DocRel] = &[
    rels_part(
        "rId1",
        "styles",
        "styles.xml",
        "/word/styles.xml",
        MT_STYLES,
    ),
    rels_part(
        "rId2",
        "numbering",
        "numbering.xml",
        "/word/numbering.xml",
        MT_NUMBERING,
    ),
    rels_part(
        "rId3",
        "footnotes",
        "footnotes.xml",
        "/word/footnotes.xml",
        MT_FOOTNOTES,
    ),
    rels_part(
        "rId4",
        "settings",
        "settings.xml",
        "/word/settings.xml",
        MT_SETTINGS,
    ),
    rels_part(
        "rId5",
        "theme",
        "theme/theme1.xml",
        "/word/theme/theme1.xml",
        MT_THEME,
    ),
    rels_part(
        "rId6",
        "header",
        "header1.xml",
        "/word/header1.xml",
        MT_HEADER,
    ),
    rels_part(
        "rId7",
        "footer",
        "footer1.xml",
        "/word/footer1.xml",
        MT_FOOTER,
    ),
    rels_part(
        "rId8",
        "comments",
        "comments.xml",
        "/word/comments.xml",
        MT_COMMENTS,
    ),
    rels_part(
        "rId9",
        "image",
        "media/image1.png",
        "/word/media/image1.png",
        "image/png",
    ),
    DocRel {
        r_id: "rId10",
        kind: "hyperlink",
        target: "mailto:jordan@example.com",
        part: None,
        media_type: "",
        external: true,
    },
    DocRel {
        r_id: "rId11",
        kind: "hyperlink",
        target: "https://www.linkedin.com/in/jrivera",
        part: None,
        media_type: "",
        external: true,
    },
];

/// Const helper: an internal document-part relationship + its part.
const fn rels_part(
    r_id: &'static str,
    kind: &'static str,
    target: &'static str,
    part: &'static str,
    media_type: &'static str,
) -> DocRel {
    DocRel {
        r_id,
        kind,
        target,
        part: Some(part),
        media_type,
        external: false,
    }
}

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("assets/acid2");

    let mut pkg = Package::new();

    // ── Main document part + the package root relationship ──────────────────
    let doc_part = PartName::new("/word/document.xml").expect("valid part name");
    pkg.set_part(
        doc_part.clone(),
        PartData::new(read(&src, "word/document.xml"), MT_DOCUMENT),
    );
    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".into(),
            rel_type: rel("officeDocument"),
            target: "word/document.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .expect("root rel");

    // ── Document-linked parts + their relationships ─────────────────────────
    let ct = pkg.content_type_map_mut();
    ct.add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    ct.add_default("xml", "application/xml");
    ct.add_default("png", "image/png");
    ct.add_override(&doc_part, MT_DOCUMENT);

    for dr in DOC_RELS {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: dr.r_id.into(),
                rel_type: rel(dr.kind),
                target: dr.target.into(),
                target_mode: if dr.external {
                    TargetMode::External
                } else {
                    TargetMode::Internal
                },
            })
            .expect("doc rel");
        if let Some(name) = dr.part {
            let part = PartName::new(name).expect("valid part name");
            let bytes = if name.ends_with("image1.png") {
                figure_png()
            } else {
                // `/word/foo.xml` → assets path `word/foo.xml`.
                read(&src, name.trim_start_matches('/'))
            };
            pkg.set_part(part.clone(), PartData::new(bytes, dr.media_type));
            if dr.media_type.ends_with("+xml") {
                pkg.content_type_map_mut()
                    .add_override(&part, dr.media_type);
            }
        }
    }

    // ── Core metadata ───────────────────────────────────────────────────────
    let core = pkg.core_properties_mut();
    core.title = Some("Loki ACID 2 — Rendering Fidelity Suite".into());
    core.creator = Some("AppThere Loki".into());
    core.subject = Some("Rendering parity stress test".into());

    // ── Normalise for Word, then write + mirror into the conformance corpus ──
    // The reviewable source parts under `assets/acid2/` are authored in a
    // convenient order, which is NOT necessarily the strict `xsd:sequence` order
    // that Microsoft Word enforces on open (tolerant readers like Loki accept
    // any order). Run Loki's own DOCX repair pass over the packaged bytes so the
    // emitted fixture is schema-ordered and opens cleanly in Word — and so this
    // generator dogfoods `loki_ooxml::repair_docx`.
    let mut buf = std::io::Cursor::new(Vec::new());
    pkg.write(&mut buf).expect("serialize package");
    let (bytes, report) = loki_ooxml::repair_docx(&buf.into_inner()).expect("repair");
    if !report.is_clean() {
        println!(
            "normalised {} out-of-order container(s) for Word compatibility",
            report.findings.len()
        );
    }

    let asset = manifest.join("assets/acid2-docx.docx");
    std::fs::write(&asset, &bytes).expect("write docx");
    let mirror = manifest.join("../appthere-conformance/fixtures/docx/acid2-docx.docx");
    std::fs::copy(&asset, &mirror).expect("mirror into fixtures");

    println!("wrote {}", asset.display());
    println!("mirrored {}", mirror.display());
}

/// Read a UTF-8 XML part from the `assets/acid2/` source tree.
fn read(src: &Path, rel_path: &str) -> Vec<u8> {
    let p: PathBuf = src.join(rel_path);
    std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// A small, deterministic bar-chart PNG used as the report figure and the
/// newsletter sidebar image. Solid bars on white — no randomness, so the
/// fixture bytes are stable across rebuilds.
fn figure_png() -> Vec<u8> {
    let (w, h) = (480u32, 288u32);
    let mut img = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let bars = [
        (0.55f32, Rgb([68, 114, 196])),
        (0.80, Rgb([237, 125, 49])),
        (0.40, Rgb([112, 173, 71])),
        (0.95, Rgb([91, 155, 213])),
        (0.65, Rgb([255, 192, 0])),
    ];
    let n = bars.len() as u32;
    let gap = 16u32;
    let bar_w = (w - gap * (n + 1)) / n;
    for (i, (frac, color)) in bars.iter().enumerate() {
        let x0 = gap + i as u32 * (bar_w + gap);
        let bar_h = (*frac * (h as f32 - 40.0)) as u32;
        let y0 = h - 20 - bar_h;
        for y in y0..(h - 20) {
            for x in x0..(x0 + bar_w) {
                img.put_pixel(x, y, *color);
            }
        }
    }
    // Baseline axis.
    for x in 0..w {
        img.put_pixel(x, h - 20, Rgb([120, 120, 120]));
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .expect("encode png");
    out.into_inner()
}
