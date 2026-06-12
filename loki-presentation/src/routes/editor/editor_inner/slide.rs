// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Slide data model.

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Slide {
    pub(super) title: String,
    pub(super) subtitle: String,
    pub(super) bullets: Vec<String>,
    pub(super) background_color: String,
    pub(super) text_color: String,
}

impl Default for Slide {
    fn default() -> Self {
        Self {
            title: "New Slide".to_string(),
            subtitle: "Double click to edit subtitle".to_string(),
            bullets: vec!["Point 1".to_string(), "Point 2".to_string()],
            background_color: "#FFFFFF".to_string(),
            text_color: "#1A1A1A".to_string(),
        }
    }
}
