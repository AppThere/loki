// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The public [`VbaProject`] model and the reader that extracts module source
//! from a `vbaProject.bin` compound file.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use encoding_rs::Encoding;

use crate::decompress::decompress;
use crate::dir;
use crate::error::{VbaError, VbaResult};
use crate::tamper;

/// A parsed VBA project — **source text only**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaProject {
    /// The project's code page (used to decode module source).
    pub code_page: u16,
    /// The modules, in `dir`-stream order.
    pub modules: Vec<VbaModule>,
    /// A tamper warning if the project appears VBA-stomped (source wiped while
    /// compiled p-code remains); `None` if it looks intact.
    pub tamper: Option<String>,
}

/// One VBA module's extracted source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaModule {
    /// The module name (e.g. `Module1`, `ThisDocument`).
    pub name: String,
    /// The module kind.
    pub kind: ModuleKind,
    /// The decompressed, decoded source text (LF line endings). Empty if the
    /// module has no recoverable source (missing, stomped, or unreadable).
    pub source: String,
}

/// The kind of a VBA module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    /// A standard (procedural) code module.
    Standard,
    /// A document/class module (`ThisDocument`, `Sheet1`, class modules).
    Document,
}

impl VbaProject {
    /// Reads a `vbaProject.bin` buffer and extracts module source.
    ///
    /// Only the decompressed source streams are read; the compiled p-code
    /// (`PerformanceCache`, `_VBA_PROJECT`) is never parsed (macro spec §4.4).
    ///
    /// # Errors
    ///
    /// [`VbaError`] if the container is not a compound file, or the `dir`
    /// stream is missing/malformed. Individual unreadable modules degrade to
    /// empty source rather than failing the whole read.
    pub fn read(bytes: &[u8]) -> VbaResult<VbaProject> {
        let mut comp = cfb::CompoundFile::open(Cursor::new(bytes))
            .map_err(|e| VbaError::Container(e.to_string()))?;

        let dir_path = find_dir_stream(&comp)
            .ok_or_else(|| VbaError::Container("no `dir` stream found".into()))?;
        let vba_storage = dir_path
            .parent()
            .map_or_else(|| PathBuf::from("/"), Path::to_path_buf);

        let dir_raw = read_stream(&mut comp, &dir_path)?;
        let info = dir::parse(&decompress(&dir_raw)?)?;

        let mut modules = Vec::with_capacity(info.modules.len());
        let mut probes = Vec::with_capacity(info.modules.len());
        for dm in &info.modules {
            let stream_name = dir::decode_mbcs(&dm.stream_name, info.code_page);
            let path = vba_storage.join(&stream_name);
            let raw = read_stream(&mut comp, &path).unwrap_or_default();
            let source = extract_source(&raw, dm.text_offset, info.code_page);
            probes.push(tamper::ModuleProbe {
                source_empty: source.trim().is_empty(),
                pcode_size: dm.text_offset.min(raw.len()),
            });
            let name = if dm.name.is_empty() {
                stream_name
            } else {
                dm.name.clone()
            };
            modules.push(VbaModule {
                name,
                kind: dm.kind,
                source,
            });
        }

        Ok(VbaProject {
            code_page: info.code_page,
            tamper: tamper::assess(&probes),
            modules,
        })
    }
}

/// Decompresses a module stream from `text_offset` (the bytes before it are the
/// ignored compiled cache) and decodes the source. Any failure → empty source.
fn extract_source(raw: &[u8], text_offset: usize, code_page: u16) -> String {
    let Some(compressed) = raw.get(text_offset..) else {
        return String::new();
    };
    match decompress(compressed) {
        Ok(bytes) => {
            let (text, _, _) = encoding_for(code_page).decode(&bytes);
            text.replace("\r\n", "\n")
        }
        Err(_) => String::new(),
    }
}

fn read_stream(comp: &mut cfb::CompoundFile<Cursor<&[u8]>>, path: &Path) -> VbaResult<Vec<u8>> {
    let mut stream = comp
        .open_stream(path)
        .map_err(|e| VbaError::Container(e.to_string()))?;
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .map_err(|e| VbaError::Container(e.to_string()))?;
    Ok(buf)
}

/// Finds the `dir` stream by walking the compound file (its parent storage is
/// the VBA storage, wherever it lives).
fn find_dir_stream(comp: &cfb::CompoundFile<Cursor<&[u8]>>) -> Option<PathBuf> {
    comp.walk()
        .find(|e| e.is_stream() && e.name().eq_ignore_ascii_case("dir"))
        .map(|e| e.path().to_path_buf())
}

/// Maps a code page to an [`Encoding`], defaulting to Windows-1252.
pub(crate) fn encoding_for(code_page: u16) -> &'static Encoding {
    match code_page {
        65001 => encoding_rs::UTF_8,
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        874 => encoding_rs::WINDOWS_874,
        932 => encoding_rs::SHIFT_JIS,
        936 => encoding_rs::GBK,
        949 => encoding_rs::EUC_KR,
        950 => encoding_rs::BIG5,
        _ => encoding_rs::WINDOWS_1252,
    }
}
