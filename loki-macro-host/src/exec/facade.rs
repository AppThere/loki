// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The object-model facade (macro spec §6.1): the `Application` and
//! `ActiveDocument` surface a text macro can touch, mapped onto the neutral
//! document. Reads are gated by `DocRead`, writes accumulate into the run's
//! single [`super::EditBatch`] behind `DocWrite`.
//!
//! v1 is a deliberately small, honest slice — `Application.Name`,
//! `Document.Name`/`.Text`/`.Content`/`.ParagraphCount`, the append/insert
//! methods, `.Text =`, and `.PrintOut`. The richer `Range`/`Selection`/
//! `Paragraphs`/`Find` surface is Phase 6; an unknown member is a clean
//! "object doesn't support this" (438), never a silent success.

use loki_basic::{RuntimeError, Value};

use super::{
    APP, DOC, DocEdit, ExecutionHost, FIND, HTTP_RESPONSE_BASE, MacroBackend, REPLACEMENT,
    SELECTION, find,
};
use crate::capability::Capability;

/// Property read (`args` empty) or method call (`args` non-empty) on a facade
/// object.
pub(super) fn get_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    obj: loki_basic::ObjectRef,
    name: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let member = name.to_ascii_lowercase();
    match obj {
        APP => application_member(host, &member, args),
        DOC => document_member(host, &member, args),
        SELECTION => find::selection_member(host, &member, args),
        FIND => find::find_member(host, &member, args),
        REPLACEMENT => find::replacement_member(host, &member, args),
        _ if obj.0 >= HTTP_RESPONSE_BASE => response_member(host, obj, &member, args),
        _ => Err(no_member()),
    }
}

/// Property assignment (`obj.Name = value`) on a facade object.
pub(super) fn set_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    obj: loki_basic::ObjectRef,
    name: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    let member = name.to_ascii_lowercase();
    match (obj, member.as_str()) {
        // `Document.Text = "…"` / `.Content = "…"` — a full-body replace.
        (DOC, "text" | "content") => {
            host.gate(Capability::DocWrite)?;
            let s = value.to_basic_string()?;
            host.doc_mut().text = s.clone();
            host.doc_mut().batch.edits.push(DocEdit::SetText(s));
            Ok(())
        }
        // `Find`/`Replacement` search parameters — no document content is
        // touched by *setting* them (the search runs at `Execute`).
        (FIND, "text") => {
            host.doc_mut().find.text = value.to_basic_string()?;
            Ok(())
        }
        (FIND, "matchcase") => {
            host.doc_mut().find.match_case = value.to_bool()?;
            Ok(())
        }
        (FIND, "wholeword") => {
            host.doc_mut().find.whole_word = value.to_bool()?;
            Ok(())
        }
        (REPLACEMENT, "text") => {
            host.doc_mut().find.replacement = Some(value.to_basic_string()?);
            Ok(())
        }
        _ => Err(no_member()),
    }
}

/// `Application.*` members. App identity is not document content, so those read
/// without a capability; `HttpGet` gates the `Network` capability per origin.
fn application_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    member: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match member {
        // App identity is not document content — always readable.
        "name" => Ok(Value::Str("Loki".into())),
        "version" => Ok(Value::Str(env!("CARGO_PKG_VERSION").into())),
        // The host document, reachable via the application.
        "activedocument" | "thisdocument" | "thiscomponent" | "activeworkbook" | "thisworkbook" => {
            Ok(Value::Object(DOC))
        }
        // `Application.HttpGet(url)` — the read-only network verb (ADR-0015 §4.1).
        "httpget" => host.http_get(arg_string(args, 0)?),
        _ => Err(no_member()),
    }
}

/// `HttpResponse.*` members — reading a fetched response. The network fetch was
/// the gated act (ADR-0015 §4.1); reading the bytes it returned needs no further
/// capability.
fn response_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    obj: loki_basic::ObjectRef,
    member: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let index = (obj.0 - HTTP_RESPONSE_BASE) as usize; // obj.0 is u32
    let response = host.doc().responses.get(index).ok_or_else(no_member)?;
    match member {
        "status" | "statuscode" => Ok(Value::from_i64_fit(i64::from(response.status))),
        "text" | "body" | "responsetext" => Ok(Value::Str(response.body_as_string())),
        "header" => {
            let name = arg_string(args, 0)?;
            Ok(Value::Str(response.header(&name).unwrap_or("").to_owned()))
        }
        _ => Err(no_member()),
    }
}

/// `Document.*` members (properties + methods).
fn document_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    member: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match member {
        // ── Reads (DocRead) ──────────────────────────────────────────────────
        "name" => {
            host.gate(Capability::DocRead)?;
            Ok(Value::Str(host.doc().title.clone()))
        }
        "text" | "content" => {
            host.gate(Capability::DocRead)?;
            Ok(Value::Str(host.doc().text.clone()))
        }
        // `Document.Range` → the whole-document range (for `.Find`). Returning
        // the object handle reads no content, so it needs no capability.
        "range" => Ok(Value::Object(super::SELECTION)),
        "paragraphcount" => {
            host.gate(Capability::DocRead)?;
            let n = host.doc().text.lines().count().max(1);
            Ok(Value::from_i64_fit(n as i64))
        }
        // ── Writes (DocWrite) ────────────────────────────────────────────────
        "appendtext" | "inserttext" | "typetext" => {
            host.gate(Capability::DocWrite)?;
            let s = arg_string(args, 0)?;
            host.doc_mut().text.push_str(&s);
            host.doc_mut().batch.edits.push(DocEdit::AppendText(s));
            Ok(Value::Empty)
        }
        // ── Print (Print) ────────────────────────────────────────────────────
        "printout" | "print" => {
            host.gate(Capability::Print)?;
            host.doc_mut().printed = true;
            Ok(Value::Empty)
        }
        _ => Err(no_member()),
    }
}

/// The `n`th argument coerced to a string, or `""` if absent.
fn arg_string(args: &[Value], n: usize) -> Result<String, RuntimeError> {
    match args.get(n) {
        Some(v) => v.to_basic_string(),
        None => Ok(String::new()),
    }
}

/// The standard "object doesn't support this property or method" error (438).
fn no_member() -> RuntimeError {
    RuntimeError::new(438, "Object doesn't support this property or method")
}
