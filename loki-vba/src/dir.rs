// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parsing of the decompressed `dir` stream (`[MS-OVBA] §2.3.4.2`).
//!
//! The `dir` stream is a flat sequence of `(Id: u16, Size: u32, data[Size])`
//! records describing the project: its code page and, per module, the module
//! name, the storage stream that holds its source, the offset of the source
//! text within that stream, and the module type. We read only the fields the
//! source viewer needs and skip everything else generically.

use crate::error::{VbaError, VbaResult};
use crate::project::ModuleKind;

/// The subset of the `dir` stream we care about.
pub(crate) struct DirInfo {
    pub(crate) code_page: u16,
    pub(crate) modules: Vec<DirModule>,
}

/// A module descriptor from the `dir` stream.
pub(crate) struct DirModule {
    pub(crate) name: String,
    pub(crate) stream_name: Vec<u8>,
    pub(crate) text_offset: usize,
    pub(crate) kind: ModuleKind,
}

// Record identifiers we act on.
const ID_CODEPAGE: u16 = 0x0003;
const ID_MODULENAME: u16 = 0x0019;
const ID_MODULESTREAMNAME: u16 = 0x001A;
const ID_MODULETYPE_PROC: u16 = 0x0021;
const ID_MODULETYPE_DOC: u16 = 0x0022;
const ID_MODULEOFFSET: u16 = 0x0031;
const ID_MODULETERMINATOR: u16 = 0x002B;

/// Parses the decompressed `dir` stream. Unknown records are skipped by their
/// declared size, so the scan is robust to fields we do not model.
pub(crate) fn parse(data: &[u8]) -> VbaResult<DirInfo> {
    let mut code_page: u16 = 1252;
    let mut modules = Vec::new();
    let mut cur: Option<DirModule> = None;

    let mut pos = 0usize;
    while pos + 6 <= data.len() {
        let id = u16::from_le_bytes([data[pos], data[pos + 1]]);
        let size = u32::from_le_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]])
            as usize;
        let body_start = pos + 6;
        let Some(body) = data.get(body_start..body_start + size) else {
            // A record claims more bytes than remain — stop cleanly rather than
            // over-read; we keep whatever modules parsed so far.
            break;
        };
        pos = body_start + size;

        match id {
            ID_CODEPAGE if size >= 2 => {
                code_page = u16::from_le_bytes([body[0], body[1]]);
            }
            ID_MODULENAME => {
                // A new module record begins; flush any in-progress one.
                if let Some(m) = cur.take() {
                    modules.push(m);
                }
                cur = Some(DirModule {
                    name: decode_mbcs(body, code_page),
                    stream_name: Vec::new(),
                    text_offset: 0,
                    kind: ModuleKind::Standard,
                });
            }
            ID_MODULESTREAMNAME => {
                if let Some(m) = cur.as_mut() {
                    m.stream_name = body.to_vec();
                }
            }
            ID_MODULEOFFSET if size >= 4 => {
                if let Some(m) = cur.as_mut() {
                    m.text_offset =
                        u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
                }
            }
            ID_MODULETYPE_PROC => {
                if let Some(m) = cur.as_mut() {
                    m.kind = ModuleKind::Standard;
                }
            }
            ID_MODULETYPE_DOC => {
                if let Some(m) = cur.as_mut() {
                    m.kind = ModuleKind::Document;
                }
            }
            ID_MODULETERMINATOR => {
                if let Some(m) = cur.take() {
                    modules.push(m);
                }
            }
            _ => {}
        }
    }
    if let Some(m) = cur.take() {
        modules.push(m);
    }
    if modules.is_empty() && data.is_empty() {
        return Err(VbaError::Directory("empty dir stream".into()));
    }
    Ok(DirInfo { code_page, modules })
}

/// Returns a copy of the decompressed `dir` stream with every module's
/// `MODULEOFFSET` (`0x0031`) set to zero — used by source-only write-back
/// (macro spec §3.4): once a module stream holds *only* its compressed source
/// (the compiled p-code prefix dropped), the source begins at offset 0.
///
/// Records are walked exactly as [`parse`] does, so an offset field is patched
/// only where it genuinely is one; malformed tails are left untouched.
pub(crate) fn zero_module_offsets(decompressed: &[u8]) -> Vec<u8> {
    let mut out = decompressed.to_vec();
    let mut pos = 0usize;
    while pos + 6 <= out.len() {
        let id = u16::from_le_bytes([out[pos], out[pos + 1]]);
        let size =
            u32::from_le_bytes([out[pos + 2], out[pos + 3], out[pos + 4], out[pos + 5]]) as usize;
        let body_start = pos + 6;
        let Some(end) = body_start.checked_add(size).filter(|&e| e <= out.len()) else {
            break;
        };
        if id == ID_MODULEOFFSET && size >= 4 {
            out[body_start..body_start + 4].fill(0);
        }
        pos = end;
    }
    out
}

/// Decodes a byte slice from the project code page to a Rust `String`, used for
/// module/stream names in the `dir` stream.
pub(crate) fn decode_mbcs(bytes: &[u8], code_page: u16) -> String {
    let encoding = crate::project::encoding_for(code_page);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}
