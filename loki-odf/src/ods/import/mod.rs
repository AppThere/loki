// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODS importer — split into focused submodules.

mod importer;
mod styles;
mod xml_helpers;

pub use importer::{OdsImport, OdsImportOptions, OdsImportResult};
