// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loro CRDT bridge for the presentation model.
//!
//! Presentation metadata and the slide list are stored structurally (a map and
//! a movable list of slide maps). Each slide's `drawing` and `placeholders` are
//! stored as a JSON snapshot string — coarse-grained but lossless, giving
//! working persistence and slide-level undo for the MVP. Fine-grained
//! per-shape CRDT containers are a planned follow-up (mirroring how
//! `loki-doc-model` started with opaque block snapshots).

use loki_graphics::{Drawing, Size};
use loro::{LoroDoc, LoroMap};

use crate::presentation::Presentation;
use crate::slide::{Placeholder, Slide, SlideId};

/// Top-level Loro container key for document metadata.
pub const KEY_METADATA: &str = "metadata";
/// Top-level Loro container key for the slide list.
pub const KEY_SLIDES: &str = "slides";

/// Errors from converting between the model and a [`LoroDoc`].
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// An error from the Loro CRDT library.
    #[error("Loro error: {0}")]
    Loro(String),
    /// A JSON (de)serialisation error for a slide snapshot.
    #[error("JSON error: {0}")]
    Json(String),
}

impl From<loro::LoroError> for BridgeError {
    fn from(e: loro::LoroError) -> Self {
        BridgeError::Loro(e.to_string())
    }
}

impl From<serde_json::Error> for BridgeError {
    fn from(e: serde_json::Error) -> Self {
        BridgeError::Json(e.to_string())
    }
}

fn get_str(map: &LoroMap, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

fn get_f64(map: &LoroMap, key: &str) -> Option<f64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_double().ok())
}

/// Serialises a [`Presentation`] into a fresh [`LoroDoc`].
pub fn presentation_to_loro(p: &Presentation) -> Result<LoroDoc, BridgeError> {
    let doc = LoroDoc::new();

    let meta = doc.get_map(KEY_METADATA);
    if let Some(title) = &p.meta.title {
        meta.insert("title", title.as_str())?;
    }
    if let Some(author) = &p.meta.author {
        meta.insert("author", author.as_str())?;
    }
    meta.insert("slide_w", p.slide_size.width)?;
    meta.insert("slide_h", p.slide_size.height)?;

    let slides = doc.get_list(KEY_SLIDES);
    for (i, slide) in p.slides.iter().enumerate() {
        let m = slides.insert_container(i, LoroMap::new())?;
        m.insert("id", slide.id.as_str())?;
        if let Some(name) = &slide.name {
            m.insert("name", name.as_str())?;
        }
        if let Some(notes) = &slide.notes {
            m.insert("notes", notes.as_str())?;
        }
        m.insert("drawing", serde_json::to_string(&slide.drawing)?.as_str())?;
        m.insert(
            "placeholders",
            serde_json::to_string(&slide.placeholders)?.as_str(),
        )?;
    }

    Ok(doc)
}

/// Reconstructs a [`Presentation`] from a [`LoroDoc`].
pub fn loro_to_presentation(doc: &LoroDoc) -> Result<Presentation, BridgeError> {
    let meta = doc.get_map(KEY_METADATA);
    let slide_size = Size::new(
        get_f64(&meta, "slide_w").unwrap_or(Presentation::WIDESCREEN_16_9.width),
        get_f64(&meta, "slide_h").unwrap_or(Presentation::WIDESCREEN_16_9.height),
    );

    let mut p = Presentation::new(slide_size);
    p.meta.title = get_str(&meta, "title");
    p.meta.author = get_str(&meta, "author");

    let slides = doc.get_list(KEY_SLIDES);
    for i in 0..slides.len() {
        let Some(m) = slides
            .get(i)
            .and_then(|v| v.into_container().ok())
            .and_then(|c| c.into_map().ok())
        else {
            continue;
        };

        let id = get_str(&m, "id").unwrap_or_else(|| format!("slide{}", i + 1));
        let drawing: Drawing = match get_str(&m, "drawing") {
            Some(s) if !s.is_empty() => serde_json::from_str(&s)?,
            _ => Drawing::new(slide_size),
        };
        let placeholders: Vec<Placeholder> = match get_str(&m, "placeholders") {
            Some(s) if !s.is_empty() => serde_json::from_str(&s)?,
            _ => Vec::new(),
        };

        p.slides.push(Slide {
            id: SlideId::new(id),
            name: get_str(&m, "name"),
            drawing,
            placeholders,
            notes: get_str(&m, "notes"),
        });
    }

    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide::PlaceholderKind;
    use loki_graphics::{Fill, RectF, Shape, TextBody};

    fn sample() -> Presentation {
        let mut p = Presentation::new(Presentation::WIDESCREEN_16_9);
        p.meta.title = Some("Quarterly Review".to_string());
        p.meta.author = Some("Ada".to_string());

        let mut slide = Slide::new("slide1", p.slide_size);
        slide.name = Some("Title slide".to_string());
        slide.notes = Some("remember to smile".to_string());
        let mut title = Shape::text_box(
            "title",
            RectF::new(40.0, 40.0, 880.0, 120.0),
            TextBody::plain("Hello"),
        );
        title.name = Some("Title".to_string());
        slide.drawing.background = Fill::None;
        slide.drawing.push(title);
        slide.add_placeholder(PlaceholderKind::Title, "title");
        p.add_slide(slide);

        // a second, empty slide
        p.add_slide(Slide::new("slide2", p.slide_size));
        p
    }

    #[test]
    fn round_trip_preserves_presentation() {
        let original = sample();
        let doc = presentation_to_loro(&original).expect("to_loro");
        let restored = loro_to_presentation(&doc).expect("from_loro");
        assert_eq!(restored, original);
    }

    #[test]
    fn metadata_and_size_survive() {
        let doc = presentation_to_loro(&sample()).unwrap();
        let restored = loro_to_presentation(&doc).unwrap();
        assert_eq!(restored.meta.title.as_deref(), Some("Quarterly Review"));
        assert_eq!(restored.slide_size, Presentation::WIDESCREEN_16_9);
        assert_eq!(restored.slide_count(), 2);
    }

    #[test]
    fn placeholder_and_shape_survive_snapshot() {
        let doc = presentation_to_loro(&sample()).unwrap();
        let restored = loro_to_presentation(&doc).unwrap();
        let slide = &restored.slides[0];
        assert_eq!(
            slide
                .placeholder(PlaceholderKind::Title)
                .map(|i| i.as_str()),
            Some("title")
        );
        assert_eq!(slide.drawing.shapes.len(), 1);
        assert_eq!(slide.notes.as_deref(), Some("remember to smile"));
    }

    #[test]
    fn empty_presentation_round_trips() {
        let original = Presentation::default();
        let doc = presentation_to_loro(&original).unwrap();
        let restored = loro_to_presentation(&doc).unwrap();
        assert_eq!(restored, original);
    }
}
