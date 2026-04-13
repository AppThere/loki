// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Open Packaging Conventions (ISO/IEC 29500-2:2021) container support.
//!
//! `loki-opc` implements OPC part loading and writing via ZIP.
//! It supports physical ZIP integration, semantic validation of names and relationships,
//! core properties serialization, and deviation handling for older files.
//!
//! # Digital Signatures note
//! Digital signatures (§10) are currently out of scope for v0.1.0. Signature parts
//! and their references are treated functionally as opaque sets and must not be edited.
//! Modifying these features directly will raise an `OpcError::DigitalSignaturesNotSupported`.

#![forbid(unsafe_code)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

pub mod compat;
pub mod constants;
pub mod content_types;
pub mod core_properties;
pub mod error;
pub mod package;
pub mod part;
pub mod relationships;
pub mod zip;

// Public Primary Re-exports
pub use error::{DeviationWarning, OpcError, OpcResult};
pub use package::Package;
pub use part::{PartData, PartName};
pub use relationships::{Relationship, RelationshipSet, TargetMode};
pub use content_types::ContentTypeMap;
pub use core_properties::CoreProperties;
