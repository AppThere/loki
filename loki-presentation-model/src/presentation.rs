// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The top-level presentation document model.

use loki_graphics::Size;
use serde::{Deserialize, Serialize};

use crate::slide::Slide;

/// Document-level metadata.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresentationMeta {
    /// Presentation title.
    pub title: Option<String>,
    /// Author / creator.
    pub author: Option<String>,
}

/// A presentation: metadata, a slide size, and an ordered deck of slides.
///
/// The slide size is the canonical page size for the deck; each [`Slide`]'s
/// drawing carries the same size so slides remain self-describing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Presentation {
    /// Document metadata.
    pub meta: PresentationMeta,
    /// Canonical slide size in points.
    pub slide_size: Size,
    /// Slides in presentation order.
    pub slides: Vec<Slide>,
}

impl Presentation {
    /// Widescreen 16:9 slide size (13.333in × 7.5in) in points.
    pub const WIDESCREEN_16_9: Size = Size::new(960.0, 540.0);

    /// Standard 4:3 slide size (10in × 7.5in) in points.
    pub const STANDARD_4_3: Size = Size::new(720.0, 540.0);

    /// An empty presentation with the given slide size.
    pub fn new(slide_size: Size) -> Self {
        Self {
            meta: PresentationMeta::default(),
            slide_size,
            slides: Vec::new(),
        }
    }

    /// Appends a slide to the deck.
    pub fn add_slide(&mut self, slide: Slide) {
        self.slides.push(slide);
    }

    /// Number of slides.
    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
}

impl Default for Presentation {
    fn default() -> Self {
        Self::new(Self::WIDESCREEN_16_9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide::Slide;

    #[test]
    fn default_is_widescreen_and_empty() {
        let p = Presentation::default();
        assert_eq!(p.slide_size, Presentation::WIDESCREEN_16_9);
        assert_eq!(p.slide_count(), 0);
    }

    #[test]
    fn add_slides() {
        let mut p = Presentation::new(Presentation::STANDARD_4_3);
        p.add_slide(Slide::new("s1", p.slide_size));
        p.add_slide(Slide::new("s2", p.slide_size));
        assert_eq!(p.slide_count(), 2);
        assert_eq!(p.slides[1].id.as_str(), "s2");
    }
}
