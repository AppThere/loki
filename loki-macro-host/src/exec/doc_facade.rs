// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The working state of the object-model facade during a run — the document
//! body + edits, and the per-run object-handle tables (responses, picked files,
//! write files). Split from `exec/mod.rs` for the 300-line ceiling.

use loki_basic::ObjectRef;

use super::edit::EditBatch;
use super::find::FindState;
use super::{FILE_HANDLE_BASE, HTTP_RESPONSE_BASE, WRITE_FILE_BASE};

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
    /// `.Write`/`.WriteLine` text; `.Close` flushes it to `path`.
    pub(crate) write_files: Vec<WriteFileState>,
}

/// The state of one `Application.OpenFileForWriting` handle: the picker-chosen
/// target, the buffered text, and whether it has been flushed.
pub(crate) struct WriteFileState {
    /// The save target the user picked (the consent, T3). Display / echo + the
    /// flush destination.
    pub(crate) path: String,
    /// Text accumulated by `.Write`/`.WriteLine`, flushed on `.Close`.
    pub(crate) buffer: String,
    /// Whether `.Close` has already flushed this handle (idempotent close).
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
        }
    }

    /// Stores `response` and returns its object handle.
    pub(crate) fn push_response(&mut self, response: crate::http::HttpResponse) -> ObjectRef {
        let handle = ObjectRef(HTTP_RESPONSE_BASE + self.responses.len() as u32);
        self.responses.push(response);
        handle
    }

    /// Stores `file` and returns its object handle.
    pub(crate) fn push_file(&mut self, file: crate::file::PickedFile) -> ObjectRef {
        let handle = ObjectRef(FILE_HANDLE_BASE + self.files.len() as u32);
        self.files.push(file);
        handle
    }

    /// Opens a write handle for the picked `path` and returns its object handle.
    pub(crate) fn push_write_file(&mut self, path: String) -> ObjectRef {
        let handle = ObjectRef(WRITE_FILE_BASE + self.write_files.len() as u32);
        self.write_files.push(WriteFileState {
            path,
            buffer: String::new(),
            closed: false,
        });
        handle
    }
}
