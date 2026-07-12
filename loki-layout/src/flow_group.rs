// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page-sharing section groups: [`flow_section_group`], split out of `flow.rs`
//! (file-ceiling pass). A *group* is one non-`continuous` section followed by
//! its `continuous` members — they share pages, with each member switching
//! column layout mid-page.

use loki_doc_model::StyleCatalog;
use loki_doc_model::layout::section::Section;

use super::{
    FlowOutput, begin_continuous_section, finish_page, flow_footnotes, flow_section,
    new_flow_state, run_paginated_loop,
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

    flow_footnotes(&mut state);
    finish_page(&mut state);
    FlowOutput::Pages {
        pages: state.pages,
        checkpoints: state.checkpoints,
        warnings: state.warnings,
    }
}
