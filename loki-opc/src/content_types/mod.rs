// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! The `[Content_Types].xml` model: per-extension default media types plus
//! per-part overrides, with override taking precedence on lookup.

mod parse;
mod write;

use std::collections::HashMap;

use crate::part::PartName;

pub use parse::parse_content_types;
pub use write::write_content_types;

/// Maps parts to their media types via extension defaults and per-part overrides.
#[derive(Debug, Clone, Default)]
pub struct ContentTypeMap {
    defaults: HashMap<String, String>,
    overrides: HashMap<PartName, String>,
}

impl ContentTypeMap {
    /// Registers a default media type for all parts with the given `extension`
    /// (matched case-insensitively).
    pub fn add_default(&mut self, extension: &str, media_type: &str) {
        self.defaults
            .insert(extension.to_ascii_lowercase(), media_type.to_string());
    }

    /// Registers a media type override for a specific `part_name`, taking
    /// precedence over the extension default.
    pub fn add_override(&mut self, part_name: &PartName, media_type: &str) {
        self.overrides
            .insert(part_name.clone(), media_type.to_string());
    }

    /// Resolves the media type for `name`: an override if present, otherwise the
    /// default for its extension, otherwise `None`.
    #[must_use]
    pub fn resolve(&self, name: &PartName) -> Option<&str> {
        if let Some(res) = self.overrides.get(name) {
            return Some(res.as_str());
        }

        if let Some(ext) = name.extension()
            && let Some(res) = self.defaults.get(&ext.to_ascii_lowercase())
        {
            return Some(res.as_str());
        }

        None
    }

    /// The extension → media-type default map (used by the serializer).
    pub(crate) fn defaults(&self) -> &HashMap<String, String> {
        &self.defaults
    }

    /// The part → media-type override map (used by the serializer).
    pub(crate) fn overrides(&self) -> &HashMap<PartName, String> {
        &self.overrides
    }
}
