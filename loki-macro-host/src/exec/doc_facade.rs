// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The working state of the object-model facade during a run — the document
//! body + edits, and the per-run object-handle tables (responses, picked files,
//! write files). Split from `exec/mod.rs` for the 300-line ceiling.

use loki_basic::ObjectRef;

use super::edit::EditBatch;
use super::find::FindState;
use super::{FILE_HANDLE_BASE, HTTP_RESPONSE_BASE, WRITE_FILE_BASE};

/// Maximum objects of each kind one run may hold.
///
/// This is a **correctness** bound, not just a resource one: the handle bases are
/// `0x1000` apart, so a table allowed to grow past 4096 entries would mint a
/// handle inside the *next* kind's range, and `facade::get_member` — which
/// dispatches highest-base-first — would then read a response as a picked file.
/// Keeping every table far below that spacing makes the ranges provably disjoint.
pub(crate) const MAX_OBJECTS_PER_KIND: usize = 256;

/// Total bytes one run may retain across fetched responses, picked files, and
/// pending write buffers. Each item is individually capped, but the *count* was
/// not — a loop fetching from a single granted origin could still exhaust memory.
pub(crate) const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

// Compile-time proof of the disjointness `facade::get_member`'s
// highest-base-first dispatch relies on: a table filled to capacity must not
// reach the next kind's base. Raising `MAX_OBJECTS_PER_KIND` past the spacing
// between the bases fails the build rather than silently making a response
// readable as a picked file.
const _: () =
    assert!(HTTP_RESPONSE_BASE as usize + MAX_OBJECTS_PER_KIND <= FILE_HANDLE_BASE as usize);
const _: () = assert!(FILE_HANDLE_BASE as usize + MAX_OBJECTS_PER_KIND <= WRITE_FILE_BASE as usize);

/// The working state of the document facade during a run.
pub(crate) struct DocFacade {
    /// The document title (`Document.Name`).
    pub(crate) title: String,
    /// The working body text — starts from the document and reflects the run's
    /// own edits so a later `.Text` read sees earlier writes.
    pub(crate) text: String,
    /// The accumulated edits (one undo entry, spec §6.2).
    pub(crate) batch: EditBatch,
    /// Whether the macro requested a print (spec §5.2 `Print`).
    pub(crate) printed: bool,
    /// The `Find`/`Replacement` search state (phase 6). A singleton backing the
    /// stateless `FIND`/`REPLACEMENT` handles.
    pub(crate) find: FindState,
    /// `HttpResponse` objects returned by `Application.HttpGet` this run, indexed
    /// by `handle - HTTP_RESPONSE_BASE` (8B.2).
    pub(crate) responses: Vec<crate::http::HttpResponse>,
    /// Picked-file objects returned by `Application.OpenFileForReading` this run,
    /// indexed by `handle - FILE_HANDLE_BASE` (Phase 7B).
    pub(crate) files: Vec<crate::file::PickedFile>,
    /// Write-file handles returned by `Application.OpenFileForWriting` this run,
    /// indexed by `handle - WRITE_FILE_BASE` (Phase 7B). The buffer accumulates
    /// `.Write`/`.WriteLine` text; `.Close` flushes it to the picked target.
    pub(crate) write_files: Vec<WriteFileState>,
    /// Bytes currently retained across the tables above, against
    /// [`MAX_RETAINED_BYTES`].
    retained_bytes: usize,
}

/// The state of one `Application.OpenFileForWriting` handle: the picker-chosen
/// target, the buffered text, and whether it has been flushed.
pub(crate) struct WriteFileState {
    /// The picked target the user chose (the consent, T3): a user-visible name
    /// plus the opaque backend handle used to perform the flush.
    pub(crate) target: crate::file::WriteTarget,
    /// Text accumulated by `.Write`/`.WriteLine`, flushed on `.Close`.
    pub(crate) buffer: String,
    /// Whether `.Close` has already flushed this handle successfully. A *failed*
    /// flush leaves this `false` so a retrying macro is not told the bytes landed.
    pub(crate) closed: bool,
}

impl DocFacade {
    /// A fresh facade for a document titled `title` with body `text`.
    pub(crate) fn new(title: String, text: String) -> Self {
        Self {
            title,
            text,
            batch: EditBatch::new(),
            printed: false,
            find: FindState::default(),
            responses: Vec::new(),
            files: Vec::new(),
            write_files: Vec::new(),
            retained_bytes: 0,
        }
    }

    /// Stores `response` and returns its object handle, or `None` if the run has
    /// hit the object-count or retained-byte budget.
    pub(crate) fn push_response(
        &mut self,
        response: crate::http::HttpResponse,
    ) -> Option<ObjectRef> {
        if self.responses.len() >= MAX_OBJECTS_PER_KIND || !self.reserve(response.body.len()) {
            return None;
        }
        let handle = ObjectRef(HTTP_RESPONSE_BASE + self.responses.len() as u32);
        self.responses.push(response);
        Some(handle)
    }

    /// Stores `file` and returns its object handle, or `None` if the run has hit
    /// the object-count or retained-byte budget.
    pub(crate) fn push_file(&mut self, file: crate::file::PickedFile) -> Option<ObjectRef> {
        if self.files.len() >= MAX_OBJECTS_PER_KIND || !self.reserve(file.bytes.len()) {
            return None;
        }
        let handle = ObjectRef(FILE_HANDLE_BASE + self.files.len() as u32);
        self.files.push(file);
        Some(handle)
    }

    /// Opens a write handle for the picked `target`, or `None` if the run has hit
    /// the object-count budget.
    pub(crate) fn push_write_file(
        &mut self,
        target: crate::file::WriteTarget,
    ) -> Option<ObjectRef> {
        if self.write_files.len() >= MAX_OBJECTS_PER_KIND {
            return None;
        }
        let handle = ObjectRef(WRITE_FILE_BASE + self.write_files.len() as u32);
        self.write_files.push(WriteFileState {
            target,
            buffer: String::new(),
            closed: false,
        });
        Some(handle)
    }

    /// Charges `len` bytes against the run's retention budget, returning whether
    /// it fits. Saturating-safe: an overflowing total is simply refused.
    pub(crate) fn reserve(&mut self, len: usize) -> bool {
        match self.retained_bytes.checked_add(len) {
            Some(total) if total <= MAX_RETAINED_BYTES => {
                self.retained_bytes = total;
                true
            }
            _ => false,
        }
    }
}
