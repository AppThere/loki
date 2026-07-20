// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

// Fixtures build little-endian records with `len as u32` casts on lengths that
// are always tiny; the narrowing is intentional.
#![allow(clippy::cast_possible_truncation)]

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::compress::compress;
use crate::project::encoding_for;
use crate::{VbaProject, write_source};

use super::MINIMAL_VBA_PROJECT;

/// A `dir` record: `id: u16`, `size: u32`, `body[size]`.
fn rec(id: u16, body: &[u8]) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(body);
    v
}

/// Compressed source (CRLF, code page 1252) as a module stream would store it.
fn source_container(lf_source: &str) -> Vec<u8> {
    let crlf = lf_source.replace('\n', "\r\n");
    let (bytes, _, _) = encoding_for(1252).encode(&crlf);
    compress(bytes.as_ref())
}

/// Builds a minimal but valid `vbaProject.bin` with the given modules. Each
/// module stream carries a fake p-code prefix (0xEE bytes) ahead of its source,
/// and the project also holds a `_VBA_PROJECT` cache and an `__SRP_0` stream —
/// exactly the artefacts source-only write-back must strip.
fn build_fixture(modules: &[(&str, &str)]) -> Vec<u8> {
    const PCODE: [u8; 16] = [0xEE; 16];
    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    comp.create_storage("/VBA").unwrap();

    let mut dir = rec(0x0003, &1252u16.to_le_bytes()); // CODEPAGE
    for (name, source) in modules {
        dir.extend(rec(0x0019, name.as_bytes())); // MODULENAME
        dir.extend(rec(0x001A, name.as_bytes())); // MODULESTREAMNAME
        dir.extend(rec(0x0021, &[])); // MODULETYPE (procedural)
        dir.extend(rec(0x0031, &(PCODE.len() as u32).to_le_bytes())); // MODULEOFFSET
        dir.extend(rec(0x002B, &[])); // module terminator

        let mut stream = PCODE.to_vec();
        stream.extend_from_slice(&source_container(source));
        write_stream(&mut comp, &format!("/VBA/{name}"), &stream);
    }
    write_stream(&mut comp, "/VBA/dir", &compress(&dir));
    write_stream(&mut comp, "/VBA/_VBA_PROJECT", &[0xAA; 40]);
    write_stream(&mut comp, "/VBA/__SRP_0", &[0xBB; 20]);

    comp.flush().unwrap();
    comp.into_inner().into_inner()
}

fn write_stream(comp: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, path: &str, content: &[u8]) {
    let mut s = comp.create_stream(path).unwrap();
    s.write_all(content).unwrap();
}

fn read_raw(bytes: &[u8], path: &str) -> Vec<u8> {
    let mut comp = cfb::CompoundFile::open(Cursor::new(bytes.to_vec())).unwrap();
    let mut s = comp.open_stream(Path::new(path)).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    buf
}

fn edits(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn fixture_reads_back_before_editing() {
    let bin = build_fixture(&[("Module1", "' original\n")]);
    let project = VbaProject::read(&bin).unwrap();
    assert_eq!(project.modules.len(), 1);
    assert_eq!(project.modules[0].source, "' original\n");
}

#[test]
fn edited_source_round_trips_through_reader() {
    let bin = build_fixture(&[("Module1", "' original\n")]);
    let out = write_source(&bin, &edits(&[("Module1", "' edited\nSub X()\nEnd Sub\n")])).unwrap();

    let project = VbaProject::read(&out).unwrap();
    assert_eq!(project.modules[0].source, "' edited\nSub X()\nEnd Sub\n");
}

#[test]
fn pcode_prefix_is_stripped_and_offset_zeroed() {
    let bin = build_fixture(&[("Module1", "' original\n")]);
    let out = write_source(&bin, &edits(&[("Module1", "' edited\n")])).unwrap();

    // The rewritten stream is a bare compressed container (starts with the 0x01
    // signature, no 0xEE p-code), equal to the edited source compressed.
    let stream = read_raw(&out, "/VBA/Module1");
    assert_eq!(stream.first(), Some(&0x01), "no p-code prefix; source at 0");
    assert!(!stream.contains(&0xEE), "fake p-code must be gone");
    assert_eq!(stream, source_container("' edited\n"));
}

#[test]
fn vba_project_cache_is_minimized_and_srp_removed() {
    let bin = build_fixture(&[("Module1", "' x\n")]);
    let out = write_source(&bin, &edits(&[])).unwrap();

    assert_eq!(read_raw(&out, "/VBA/_VBA_PROJECT"), MINIMAL_VBA_PROJECT);

    let comp = cfb::CompoundFile::open(Cursor::new(out)).unwrap();
    assert!(
        !comp.is_stream(Path::new("/VBA/__SRP_0")),
        "__SRP_ must be gone"
    );
}

#[test]
fn unedited_module_keeps_source_but_loses_pcode() {
    let bin = build_fixture(&[("Module1", "' keep me\n"), ("Module2", "' and me\n")]);
    // Edit only Module1; Module2 is carried over.
    let out = write_source(&bin, &edits(&[("Module1", "' changed\n")])).unwrap();

    let project = VbaProject::read(&out).unwrap();
    let by_name = |n: &str| {
        project
            .modules
            .iter()
            .find(|m| m.name == n)
            .map(|m| m.source.clone())
            .unwrap()
    };
    assert_eq!(by_name("Module1"), "' changed\n");
    assert_eq!(by_name("Module2"), "' and me\n");
    // Carried-over module also starts at offset 0 (no p-code).
    let stream = read_raw(&out, "/VBA/Module2");
    assert_eq!(stream.first(), Some(&0x01));
    assert!(!stream.contains(&0xEE));
}

#[test]
fn non_container_input_is_typed_error() {
    assert!(write_source(b"not a compound file", &edits(&[])).is_err());
}
