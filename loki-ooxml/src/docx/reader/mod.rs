// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! XML readers that parse OOXML part bytes into intermediate model types.
//!
//! All readers use `quick-xml` in event reader mode with `trim_text(false)`
//! to preserve whitespace. See ADR-0002.

pub mod document;
pub mod footnotes;
pub mod numbering;
pub mod settings;
pub mod styles;
pub mod util;

