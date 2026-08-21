// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page-sharing section groups: [`flow_section_group`], split out of `flow.rs`
//! (file-ceiling pass). A *group* is one non-`continuous` section followed by
//! its `continuous` members — they share pages, with each member switching
//! column layout mid-page.

use loki_doc_model::StyleCatalog;
use loki_doc_model::layout::section::Section;

use super::{
    BreakCause, FlowOutput, balance, begin_continuous_section, finish_page, new_flow_state,
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
/// A **single-section group** goes straight to the column-balancing path
/// (`flow_balance::flow_paginated_balanced`) — this is how production documents
/// (via `layout_paginated_full`) get their multi-column sections balanced. It
/// reaches that path directly rather than via `flow_section`, which is the
/// no-neighbours entry point and so seeds a zero `carry` by construction.
/// Genuinely-continuous (multi-section) groups keep the fill-first
/// flow: their tail can start mid-page inside another member, which the
/// checkpoint-based last-page balancing cannot resume (documented limitation).
///
/// Paginated mode only — the non-paginated (reflow/pageless) path flows each
/// section independently (continuous-scroll has no pages to share). Editing
/// block indices are group-local; the caller globalises them per section.
///
/// # `carry`: paragraph spacing across a `nextPage` section start
///
/// Word treats a section start exactly like a `w:pageBreakBefore` paragraph —
/// [`BreakCause::Forced`] — so the first paragraph's `space_before` collapses
/// against the *previous section's* last block's `space_after`: 36 pt requested
/// behind an 8 pt `after` gives 27.96 pt, measured.
///
/// Each `nextPage` group is a separate page sequence flowed from its own fresh
/// `FlowState`, so nothing inside one group can see the previous group's
/// trailing spacing. `carry` is that one value threaded across the boundary by
/// `layout_paginated_full`: read in to seed this group's collapse state, and
/// written back out with this group's own trailing `space_after`. It starts at
/// `0.0`, which is also what a first section correctly sees.
// Eight arguments: seven are the flow inputs every entry point in this module
// takes (and `flow_paginated_balanced`, which this delegates to, carries the
// same allow), plus `carry`. Bundling them into a context struct is a change to
// every flow entry point, not to this one.
#[allow(clippy::too_many_arguments)]
pub fn flow_section_group(
    resources: &mut FontResources,
    sections: &[&Section],
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
    options: &LayoutOptions,
    comments: &[loki_doc_model::content::annotation::Comment],
    carry: &mut f32,
) -> FlowOutput {
    debug_assert!(mode.is_paginated(), "flow_section_group is paginated-only");
    if let [only] = sections {
        // Straight to the balanced flow rather than through `flow_section`,
        // which seeds a zero carry by construction (it is the entry point for a
        // section with no neighbours). `false` is `flow_section`'s own
        // `ended_by_continuous` argument — a group reaching here is not
        // followed by a `continuous` section.
        return balance::flow_paginated_balanced(
            resources,
            only,
            catalog,
            mode,
            display_scale,
            options,
            comments,
            false,
            carry,
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
    state.last_space_after = *carry;

    let mut block_base = 0usize;
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            begin_continuous_section(&mut state, section);
        }
        run_paginated_loop(&mut state, &section.blocks, 0, block_base, |_, _| false);
        block_base += section.blocks.len();
    }

    // Read before `finish_page`, which clears it on a `Flow` break.
    *carry = state.last_space_after;
    // `finish_page` lays out the final page's footnote band (per-page placement).
    finish_page(&mut state, BreakCause::Flow);
    FlowOutput::Pages {
        pages: state.pages,
        checkpoints: state.checkpoints,
        warnings: state.warnings,
    }
}
