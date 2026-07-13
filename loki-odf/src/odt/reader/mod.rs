// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XML readers for ODF document parts.
//!
//! Each reader accepts raw XML bytes (`&[u8]`) and returns the corresponding
//! intermediate model type.  All readers set `trim_text(false)` so that
//! significant whitespace inside `text:span` and similar elements is
//! preserved verbatim.
//!
//! XXE posture (audit-2026-06 S-5): every reader parses with
//! `quick_xml::Reader`, which does not resolve external entities or fetch
//! DTDs. Do not enable DTD/entity expansion without a security review.

pub(crate) mod annotations;
pub(crate) mod columns;
pub(crate) mod document;
pub(crate) mod inlines;
pub(crate) mod meta;
pub(crate) mod revisions;
pub(crate) mod styles;
