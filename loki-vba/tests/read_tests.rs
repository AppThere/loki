// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end: synthesize a `vbaProject.bin` compound file (with real
//! MS-OVBA-compressed `dir` and module streams) and read the source back.

use std::io::{Cursor, Write};

use loki_vba::{ModuleKind, VbaProject};

/// A minimal, valid MS-OVBA "compressed" container that encodes `data` as
/// literal-only tokens (correct, if not space-efficient). Requires
/// `data.len()` small enough to fit one 4096-byte chunk.
fn ovba_literal(data: &[u8]) -> Vec<u8> {
    assert!(data.len() <= 4096);
    if data.is_empty() {
        return vec![0x01]; // signature only — decompresses to empty
    }
    let mut chunk = Vec::new();
    let mut i = 0;
    while i < data.len() {
        chunk.push(0x00u8); // flag: the next up-to-8 tokens are literals
        for _ in 0..8 {
            if i >= data.len() {
                break;
            }
            chunk.push(data[i]);
            i += 1;
        }
    }
    let header: u16 = 0x8000 | 0x3000 | ((chunk.len() as u16 - 1) & 0x0FFF);
    let mut out = vec![0x01u8];
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(&chunk);
    out
}

/// One `dir`-stream record: `Id (u16) | Size (u32) | data`.
fn rec(id: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn build_dir_stream(module: &str, offset: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(rec(0x0003, &1252u16.to_le_bytes())); // PROJECTCODEPAGE
    d.extend(rec(0x0019, module.as_bytes())); // MODULENAME
    d.extend(rec(0x001A, module.as_bytes())); // MODULESTREAMNAME
    d.extend(rec(0x0031, &offset.to_le_bytes())); // MODULEOFFSET
    d.extend(rec(0x0021, &[])); // MODULETYPE = procedural
    d.extend(rec(0x002B, &[])); // MODULETERMINATOR
    d
}

/// Assembles a `vbaProject.bin` with a `/VBA/dir` + `/VBA/<module>` stream. The
/// module stream is `[p-code prefix][compressed source]`, and `MODULEOFFSET` is
/// set to the prefix length (the prefix is the ignored compiled cache).
fn build_vba_bin(module: &str, source: &str, pcode_prefix: &[u8]) -> Vec<u8> {
    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    comp.create_storage("/VBA").unwrap();

    let dir = ovba_literal(&build_dir_stream(module, pcode_prefix.len() as u32));
    comp.create_stream("/VBA/dir")
        .unwrap()
        .write_all(&dir)
        .unwrap();

    let mut stream = pcode_prefix.to_vec();
    stream.extend(ovba_literal(source.as_bytes()));
    comp.create_stream(format!("/VBA/{module}"))
        .unwrap()
        .write_all(&stream)
        .unwrap();

    comp.flush().unwrap();
    comp.into_inner().into_inner()
}

#[test]
fn reads_module_source() {
    let src = "Sub Hello()\r\n    MsgBox \"hi\"\r\nEnd Sub";
    let bin = build_vba_bin("Module1", src, &[]);
    let project = VbaProject::read(&bin).expect("read");

    assert_eq!(project.code_page, 1252);
    assert_eq!(project.modules.len(), 1);
    let m = &project.modules[0];
    assert_eq!(m.name, "Module1");
    assert_eq!(m.kind, ModuleKind::Standard);
    // CRLF normalised to LF on extraction.
    assert_eq!(m.source, "Sub Hello()\n    MsgBox \"hi\"\nEnd Sub");
    assert!(project.tamper.is_none());
}

#[test]
fn stomped_module_raises_tamper_warning() {
    // Substantial compiled p-code prefix, but the source at MODULEOFFSET is
    // empty → the VBA-stomping heuristic fires.
    let pcode = vec![0xABu8; 400];
    let bin = build_vba_bin("Module1", "", &pcode);
    let project = VbaProject::read(&bin).expect("read");
    assert!(project.modules[0].source.is_empty());
    assert!(
        project.tamper.is_some(),
        "empty source + 400 bytes of p-code should warn"
    );
}

#[test]
fn non_compound_input_is_typed_error() {
    let err = VbaProject::read(b"not a compound file at all").unwrap_err();
    assert!(matches!(err, loki_vba::VbaError::Container(_)));
}

#[test]
fn empty_input_does_not_panic() {
    assert!(VbaProject::read(&[]).is_err());
}
