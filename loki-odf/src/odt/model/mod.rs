// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Intermediate ODF document model.
//!
//! These types are **internal only** — they mirror the XML structure of an ODF
//! package and are used as an intermediate representation between the raw XML
//! parser and the format-neutral [`loki_doc_model`] types.
//!
//! The model is deliberately close to the ODF 1.3 element tree so that
//! spec-section references remain accurate and round-trip fidelity is
//! preserved. Conversion to `loki_doc_model` types happens in a separate
//! mapping step.
//!
//! All types are defined now but consumed only by the XML parser and mapper
//! added in later sessions; the dead-code lint is suppressed at the module
//! level to keep the build warning-free.
#![allow(dead_code)]

pub(crate) mod document;
pub(crate) mod fields;
pub(crate) mod frames;
pub(crate) mod list_styles;
pub(crate) mod notes;
pub(crate) mod paragraph;
pub(crate) mod styles;
pub(crate) mod tables;
