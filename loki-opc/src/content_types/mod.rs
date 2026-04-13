// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Tracking metadata resolution parameters mapping specific extensions/paths internally isolating overrides.

mod parse;
mod write;

use std::collections::HashMap;

use crate::part::PartName;

pub use parse::parse_content_types;
pub use write::write_content_types;

/// Content type tracking structure defining explicit URI targets resolving strictly matching components.
#[derive(Debug, Clone, Default)]
pub struct ContentTypeMap {
    defaults: HashMap<String, String>,
    overrides: HashMap<PartName, String>,
}

impl ContentTypeMap {
    /// Registers fallback content mapping identifying arbitrary files relying identically upon extension definitions.
    pub fn add_default(&mut self, extension: &str, media_type: &str) {
        self.defaults.insert(extension.to_ascii_lowercase(), media_type.to_string());
    }

    /// Extends overrides matching paths precisely avoiding defaults explicitly identifying components universally.
    pub fn add_override(&mut self, part_name: &PartName, media_type: &str) {
        self.overrides.insert(part_name.clone(), media_type.to_string());
    }

    /// Evaluates resolution targeting paths exclusively providing deterministic lookup bounds tracking targets.
    #[must_use]
    pub fn resolve(&self, name: &PartName) -> Option<&str> {
        if let Some(res) = self.overrides.get(name) {
            return Some(res.as_str());
        }

        if let Some(ext) = name.extension()
            && let Some(res) = self.defaults.get(&ext.to_ascii_lowercase()) {
                return Some(res.as_str());
            }
        
        None
    }
    
    /// Provide underlying maps iterating outputs dynamically serializing properties mapping files.
    pub(crate) fn defaults(&self) -> &HashMap<String, String> {
        &self.defaults
    }
    
    /// Extract configuration overrides internally binding path parameters identifying parts implicitly.
    pub(crate) fn overrides(&self) -> &HashMap<PartName, String> {
        &self.overrides
    }
}
