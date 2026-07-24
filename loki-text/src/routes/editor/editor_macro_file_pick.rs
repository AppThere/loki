// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! UI-thread handling of a running macro's file-pick requests (macro spec §5.3,
//! Phase 7B).
//!
//! Unlike capability prompts (which render a component and wait for a click), a
//! file request drives the **async** OS picker and does the byte I/O through the
//! platform [`FileAccessToken`] — both must run on the UI thread. The macro
//! worker blocks on the reply via the bridge. `.Close` writes are performed here
//! too (not on the worker) so Android content-URI tokens are used on the right
//! thread; there is no path-addressed `std::fs` access.

use std::io::{Read, Write};

use loki_file_access::{FileAccessToken, FilePicker, PickOptions, SaveOptions};
use loki_macro_host::{FileFilter, PickedFile};

use super::editor_macro_bridge::{PendingPrompt, UiReply, UiRequest};

/// The file operation a [`PendingPrompt`] carries, extracted so the prompt's
/// borrow ends before the async picker runs.
enum FileOp {
    Read(FileFilter),
    PickWrite(FileFilter),
    Write(String, Vec<u8>),
}

/// Handle a file-pick request from the running macro, answering it on the UI
/// thread. Returns `Some(prompt)` **unchanged** if it is not a file request (so
/// the caller renders it as a normal prompt); returns `None` once handled.
pub(super) async fn try_handle_file_request(prompt: PendingPrompt) -> Option<PendingPrompt> {
    let op = match prompt.request() {
        UiRequest::PickReadFile(filter) => FileOp::Read(filter.clone()),
        UiRequest::PickWriteTarget(filter) => FileOp::PickWrite(filter.clone()),
        UiRequest::WriteFile { path, bytes } => FileOp::Write(path.clone(), bytes.clone()),
        // Not a file request — hand it back for the normal prompt path.
        _ => return Some(prompt),
    };
    let reply = match op {
        FileOp::Read(filter) => UiReply::ReadFile(pick_and_read(&filter).await),
        FileOp::PickWrite(filter) => UiReply::WritePath(pick_write_target(&filter).await),
        FileOp::Write(path, bytes) => UiReply::WriteResult(write_bytes(&path, &bytes)),
    };
    prompt.answer(reply);
    None
}

/// Raise the open picker and read the chosen file's bytes.
async fn pick_and_read(filter: &FileFilter) -> Option<PickedFile> {
    let opts = PickOptions {
        mime_types: mime_types(filter),
        filter_label: filter_label(filter),
        multi: false,
    };
    match FilePicker::new().pick_file_to_open(opts).await {
        Ok(Some(token)) => read_token(&token),
        // Cancelled (`Ok(None)`) or a picker error — the macro sees "no file".
        _ => None,
    }
}

/// Read a picked token into a [`PickedFile`], or `None` on an I/O error.
fn read_token(token: &FileAccessToken) -> Option<PickedFile> {
    let mut reader = token.open_read().ok()?;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(PickedFile {
        path: token.serialize(),
        bytes,
    })
}

/// Raise the save picker and return the chosen target as a serialized token.
async fn pick_write_target(filter: &FileFilter) -> Option<String> {
    let opts = SaveOptions {
        mime_type: mime_types(filter).into_iter().next(),
        suggested_name: filter.extensions.first().map(|ext| format!("output.{ext}")),
    };
    match FilePicker::new().pick_file_to_save(opts).await {
        Ok(Some(token)) => Some(token.serialize()),
        _ => None,
    }
}

/// Flush `bytes` to the target the user already picked (deserialized from `path`).
fn write_bytes(path: &str, bytes: &[u8]) -> Result<(), String> {
    let token = FileAccessToken::deserialize(path).map_err(|e| e.to_string())?;
    let mut writer = token.open_write().map_err(|e| e.to_string())?;
    writer.write_all(bytes).map_err(|e| e.to_string())?;
    Ok(())
}

/// Best-effort MIME types for the filter's extensions (advisory — the user's pick
/// is the authority). Unknown extensions are dropped; an all-unknown/empty filter
/// yields no MIME filter (all files shown).
fn mime_types(filter: &FileFilter) -> Vec<String> {
    filter
        .extensions
        .iter()
        .filter_map(|ext| mime_for_ext(ext))
        .map(str::to_owned)
        .collect()
}

/// A short, human-readable label for the filter, or `None` for "any file".
fn filter_label(filter: &FileFilter) -> Option<String> {
    if filter.extensions.is_empty() {
        None
    } else {
        Some(
            filter
                .extensions
                .iter()
                .map(|ext| format!("*.{ext}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

/// Common extension → MIME mapping. Deliberately small: the filter only shapes
/// the picker's file-type list, so an unmapped extension simply shows all files.
fn mime_for_ext(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "txt" | "text" | "log" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "md" | "markdown" => "text/markdown",
        "pdf" => "application/pdf",
        "rtf" => "application/rtf",
        _ => return None,
    })
}
