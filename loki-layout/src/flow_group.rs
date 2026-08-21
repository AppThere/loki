// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page-sharing section groups: [`flow_section_group`], split out of `flow.rs`
//! (file-ceiling pass). A *group* is one non-`continuous` section followed by
//! its `continuous` members — they share pages, with each member switching
//! column layout mid-page.

use loki_doc_model::StyleCatalog;
use loki_doc_model::layout::section::Section;

use super::{
    BreakCause, FlowOutput, begin_continuous_section, finish_page, flow_section, new_flow_state,
    run_paginated_loop,
};
use crate::LayoutOptions;
use crate::font::FontResources;
use crate::mode::LayoutMode;

/// Flows a **group** of sections that share pages: the first section starts the
/// page sequence, and every subsequent (`continuous`) member continues on the
/// same page, switching column layout mid-page via `begin_continuous_section`.
/// Page geometry and headers/footers come from the group's first section.
///
/// A **single-section group** delegates to [`flow_section`], which routes
/// through the column-balancing path (`flow_balance`) — this is how production
/// documents (via `layout_paginated_full`) get their multi-column sections
/// balanced. Genuinely-continuous (multi-section) groups keep the fill-first
/// flow: their tail can start mid-page inside another member, which the
/// checkpoint-based last-page balancing cannot resume (documented limitation).
///
/// Paginated mode only — the non-paginated (reflow/pageless) path flows each
/// section independently (continuous-scroll has no pages to share). Editing
/// block indices are group-local; the caller globalises them per section.
///
/// # `TODO(section-space-before-collapse)`: a `nextPage` section start applies
/// `space_before` in full
///
/// Word treats a section start exactly like a `w:pageBreakBefore` paragraph —
/// [`BreakCause::Forced`] — so the first paragraph's `space_before` collapses
/// against the *previous section's* last paragraph's `space_after`: 36 pt
/// requested behind an 8 pt `after` gives 27.96 pt, measured. Loki gives the
/// full 36, because each `nextPage` group is flowed here from a fresh
/// `FlowState` whose `last_space_after` starts at 0 — there is nothing left to
/// collapse against. Fixing it means carrying the trailing `space_after` out of
/// one group and seeding the next, which crosses the `FlowOutput` boundary and
/// so is not a rider on the `BreakCause` change. This is the ~8 pt vertical
/// offset on `acid2-docx.docx` page 7.
pub fn flow_section_group(
    resources: &mut FontResources,
    sections: &[&Section],
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
    options: &LayoutOptions,
    comments: &[loki_doc_model::content::annotation::Comment],
) -> FlowOutput {
    debug_assert!(mode.is_paginated(), "flow_section_group is paginated-only");
    if let [only] = sections {
        return flow_section(
            resources,
            only,
            catalog,
            mode,
            display_scale,
            options,
            comments,
        );
    }
    let primary = sections[0];
    let mut state = new_flow_state(
        resources,
        primary,
        catalog,
        mode,
        display_scale,
        options,
        comments,
    );

    let mut block_base = 0usize;
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            begin_continuous_section(&mut state, section);
        }
        run_paginated_loop(&mut state, &section.blocks, 0, block_base, |_, _| false);
        block_base += section.blocks.len();
    }

    // `finish_page` lays out the final page's footnote band (per-page placement).
    finish_page(&mut state, BreakCause::Flow);
    FlowOutput::Pages {
        pages: state.pages,
        checkpoints: state.checkpoints,
        warnings: state.warnings,
    }
}
