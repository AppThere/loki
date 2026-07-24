// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Source-only write-back for a VBA project (`vbaProject.bin`), macro spec §3.4.
//!
//! When the macro editor saves, edited modules are written back **source-only**:
//! the compiled p-code is dropped and Office / `LibreOffice` recompile from source on
//! next open. This is both the documented behaviour and a security property —
//! an edited project can never carry stale (possibly malicious) p-code, closing
//! threat T5 ("VBA stomping").
//!
//! The write is a **surgical patch** of the existing compound file, not a
//! from-scratch re-serialisation: everything we do not model (the `PROJECT` /
//! `PROJECTwm` streams, project references, module ordering, storage layout) is
//! preserved verbatim. Only four things change:
//!
//! 1. every module stream is rewritten to hold *only* its compressed source
//!    (the p-code prefix at `MODULEOFFSET` is dropped);
//! 2. the `dir` stream's every `MODULEOFFSET` is zeroed to match (source now
//!    begins at offset 0) and recompressed;
//! 3. `_VBA_PROJECT` (the performance cache) is replaced with a minimal header
//!    that forces a recompile;
//! 4. stale `__SRP_*` serialized-reference caches are removed.
//!
//! Like the rest of `loki-vba`, this executes nothing — it is a byte transform.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::compress::compress;
use crate::decompress::decompress;
use crate::dir::{self, DirModule};
use crate::error::{VbaError, VbaResult};
use crate::project::{encoding_for, find_dir_stream, read_stream};

/// Minimal `_VBA_PROJECT` performance-cache header (`[MS-OVBA] §2.3.4.1`):
/// `Reserved1 = 0x61CC`, `Version = 0xFFFF`, `Reserved2 = 0x00`,
/// `Reserved3 = 0x0000`, with no cached p-code. The version word is ignored on
/// read; the empty cache forces Office to recompile from the source streams —
/// the same thing `LibreOffice` writes.
const MINIMAL_VBA_PROJECT: [u8; 7] = [0xCC, 0x61, 0xFF, 0xFF, 0x00, 0x00, 0x00];

/// Rewrites `original` (a `vbaProject.bin`) with edited module source, keeping
/// only source (no compiled p-code). `edits` maps a module's name (as
/// [`crate::VbaModule::name`] reports it) to its new LF-terminated source;
/// modules absent from the map keep their existing source but still lose their
/// p-code. The returned bytes are a valid compound file ready to re-embed as the
/// preserved macro part.
///
/// # Errors
///
/// [`VbaError`] if `original` is not a readable compound file, its `dir` stream
/// is missing/malformed, or a stream cannot be rewritten.
pub fn write_source(original: &[u8], edits: &BTreeMap<String, String>) -> VbaResult<Vec<u8>> {
    let mut comp = cfb::CompoundFile::open(Cursor::new(original.to_vec()))
        .map_err(|e| VbaError::Container(e.to_string()))?;

    let dir_path = find_dir_stream(&comp)
        .ok_or_else(|| VbaError::Container("no `dir` stream found".into()))?;
    let vba_storage = dir_path
        .parent()
        .map_or_else(|| PathBuf::from("/"), Path::to_path_buf);

    let dir_decompressed = decompress(&read_stream(&mut comp, &dir_path)?)?;
    let info = dir::parse(&dir_decompressed)?;

    // 1. Rewrite each module stream to compressed-source-only.
    for m in &info.modules {
        let stream_name = dir::decode_mbcs(&m.stream_name, info.code_page);
        let path = vba_storage.join(&stream_name);
        // Tolerate a missing/unreadable stream exactly as the reader does
        // (`project::read` uses `unwrap_or_default`): an unedited module with no
        // stream degrades to empty source rather than failing the whole save.
        let existing = read_stream(&mut comp, &path).unwrap_or_default();
        let content = module_content(&existing, m, info.code_page, edits)?;
        write_stream(&mut comp, &path, &content)?;
    }

    // 2. Zero every MODULEOFFSET so the source is found at offset 0, recompress.
    let patched_dir = dir::zero_module_offsets(&dir_decompressed);
    write_stream(&mut comp, &dir_path, &compress(&patched_dir))?;

    // 3. Force a clean recompile from source.
    let vba_project = vba_storage.join("_VBA_PROJECT");
    if comp.is_stream(&vba_project) {
        write_stream(&mut comp, &vba_project, &MINIMAL_VBA_PROJECT)?;
    }

    // 4. Drop stale serialized-reference caches.
    remove_srp_streams(&mut comp)?;

    comp.flush().map_err(|e| VbaError::Write(e.to_string()))?;
    Ok(comp.into_inner().into_inner())
}

/// The new source-only content for one module stream. When `edits` names the
/// module, its source is re-encoded (LF→CRLF, project code page) and compressed;
/// otherwise the module's existing compressed source is carried over verbatim.
/// Either way the compiled p-code prefix (`existing[..text_offset]`) is dropped.
///
/// # Errors
///
/// [`VbaError::Encoding`] if edited source contains characters the project's code
/// page cannot represent — refused rather than silently corrupting the source.
fn module_content(
    existing: &[u8],
    m: &DirModule,
    code_page: u16,
    edits: &BTreeMap<String, String>,
) -> VbaResult<Vec<u8>> {
    if let Some(src) = edits.get(m.name.as_str()) {
        let crlf = src.replace("\r\n", "\n").replace('\n', "\r\n");
        let (bytes, _, had_unmappable) = encoding_for(code_page).encode(&crlf);
        if had_unmappable {
            return Err(VbaError::Encoding(m.name.clone()));
        }
        return Ok(compress(bytes.as_ref()));
    }
    Ok(match existing.get(m.text_offset..) {
        Some(tail) if !tail.is_empty() => tail.to_vec(),
        // Missing/stomped source: emit a valid empty container rather than an
        // empty stream, so a re-read decodes cleanly to empty source.
        _ => compress(b""),
    })
}

/// Replaces (or creates) a stream's entire contents.
fn write_stream<F: Read + Write + Seek>(
    comp: &mut cfb::CompoundFile<F>,
    path: &Path,
    content: &[u8],
) -> VbaResult<()> {
    let mut stream = comp
        .create_stream(path)
        .map_err(|e| VbaError::Write(e.to_string()))?;
    stream
        .write_all(content)
        .map_err(|e| VbaError::Write(e.to_string()))?;
    Ok(())
}

/// Removes every `__SRP_*` serialized-reference cache stream (a stale p-code
/// artefact; source-only projects must not carry it).
fn remove_srp_streams<F: Read + Write + Seek>(comp: &mut cfb::CompoundFile<F>) -> VbaResult<()> {
    let stale: Vec<PathBuf> = comp
        .walk()
        .filter(|e| e.is_stream() && e.name().to_ascii_uppercase().starts_with("__SRP_"))
        .map(|e| e.path().to_path_buf())
        .collect();
    for path in stale {
        comp.remove_stream(&path)
            .map_err(|e| VbaError::Write(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
