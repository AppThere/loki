// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Test-only [`FontResources`] constructors that exclude the host's fonts.
//!
//! Split from `font.rs` for the 300-line ceiling. They live in the crate rather
//! than in each test file because two things need them — the substitution tests
//! in `font_tests.rs`, whose premise is that a family is *missing*, and every
//! layout fixture that asserts a glyph position, page count or rule height and
//! must therefore measure a face from this repository rather than whatever the
//! machine calls its default sans.

use std::collections::HashMap;

use super::FontResources;
use crate::para_cache::ParaCache;

impl FontResources {
    /// A `FontResources` whose collection contains **no system fonts** — only
    /// what the workspace itself registers.
    ///
    /// Tests about missing-font behaviour cannot use [`Self::new`]: it scans the
    /// host, so "Courier is absent, therefore we substitute Courier Prime" is
    /// true on a bare CI container and false on any macOS machine (which ships
    /// Courier, Courier New, Arial, Times New Roman and Georgia). The assertion
    /// then reports the host, not the code. Here the premise holds by
    /// construction on every platform.
    ///
    /// The bundled faces are **not** pre-registered, so
    /// [`Self::ensure_fallback_fonts_registered`]'s lazy path is exercised
    /// exactly as it is on a machine with nothing installed.
    ///
    /// Note for layout fixtures: with no system fonts there is no family for an
    /// unnamed span to bind to, and Parley lays out **zero glyphs**. Any fixture
    /// using this must name a bundled family (see `loki_fonts::bundled_families`)
    /// in its `CharProps`, which is what pins its metrics across hosts in the
    /// first place.
    pub(crate) fn without_system_fonts() -> Self {
        let collection = parley::fontique::Collection::new(parley::fontique::CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        Self {
            font_cx: parley::FontContext {
                collection,
                source_cache: parley::fontique::SourceCache::default(),
            },
            layout_cx: parley::LayoutContext::new(),
            font_data_cache: HashMap::new(),
            substitutions: HashMap::new(),
            run_substitutions: HashMap::new(),
            para_cache: ParaCache::default(),
            fallbacks_registered: false,
        }
    }

    /// Like [`Self::without_system_fonts`], but with the bundled faces
    /// registered **and bound to the generic families**, so a span that names no
    /// family still lays out — deterministically, from this repository's own
    /// font bytes.
    ///
    /// This is what a layout fixture wants. `font_name: None` pushes no
    /// `FontFamily` property at all (`para_build.rs`), so Parley resolves the
    /// generic default: on a host collection that is the machine's default sans,
    /// which is why page counts and glyph positions differed between CI and
    /// macOS; on a bare hermetic collection it is *nothing*, and the fixture
    /// silently measures an empty layout. Binding the generics closes both.
    pub(crate) fn with_bundled_fonts_only() -> Self {
        use parley::fontique::GenericFamily;

        let mut this = Self::without_system_fonts();
        this.ensure_fallback_fonts_registered();

        // Carlito (Calibri-metric) is the default text face a word processor
        // would use; Tinos and Cousine cover the serif and monospace generics.
        let coll = &mut this.font_cx.collection;
        for (generic, family) in [
            (GenericFamily::SansSerif, "Carlito"),
            (GenericFamily::UiSansSerif, "Carlito"),
            (GenericFamily::SystemUi, "Carlito"),
            (GenericFamily::Serif, "Tinos"),
            (GenericFamily::UiSerif, "Tinos"),
            (GenericFamily::Monospace, "Cousine"),
            (GenericFamily::UiMonospace, "Cousine"),
        ] {
            if let Some(id) = coll.family_id(family) {
                coll.set_generic_families(generic, [id].into_iter());
            }
        }
        this
    }
}
