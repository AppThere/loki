// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph placement, splitting, and keep-with-next chain logic.
//!
//! Split algorithm (ADR 004 §3): when a paragraph does not fit on the current
//! page, it is split at the last Parley line boundary that fits. Each fragment
//! is wrapped in a [`PositionedItem::ClippedGroup`] so that full-height
//! background and border items are clipped correctly.
//!
//! # Session 3 pre-audit findings (2026-04-20)
//!
//! ## Q1 — indent_hanging coverage (55f489b)
//! `indent_hanging` is applied in `layout_paragraph` for ALL paragraphs
//! (not just list items): the glyph-run loop shifts line 0 left by
//! `indent_hanging` unconditionally when the field is > 0. Non-list paragraphs
//! with a manually set `indent_hanging` therefore produce the correct first-line
//! offset. One known gap: `line_w` passed to `break_all_lines` is computed as
//! `available_width − indent_start − indent_end` for every line, so Parley
//! wraps line 0 at the same column as continuation lines. The first line
//! physically starts `indent_hanging` to the left but wraps `indent_hanging` too
//! early. Fixing this requires per-line width, which Parley 0.6 does not expose;
//! the inaccuracy is minor (≤ one word per line) and non-blocking for Session 3.
//!
//! ## Q2 — Parley 0.6 bidi API
//! `BidiLevel` and `BidiResolver` are `pub(crate)` — no public API exists to
//! set a per-paragraph base direction. There is no `StyleProperty` variant for
//! text direction in Parley 0.6's `StyleProperty` enum
//! (`FontStack`, `FontSize`, `FontStyle`, `FontWeight`, `Underline`,
//! `Strikethrough`, `LineHeight`, `WordSpacing`, `LetterSpacing`, `WordBreak`,
//! `OverflowWrap`, `Locale` — no RTL/bidi entry). Parley runs the Unicode BiDi
//! algorithm automatically on character class properties. Gap #5 (RTL paragraph
//! direction) cannot be addressed via `StyleProperty`; the only workaround is
//! embedding Unicode directional control characters (U+202B RLE / U+200F RLM)
//! into the text string. Defer to a future Parley version or a separate session.
//!
//! ## Q3 — page_break_after hook point
//! `page_break_after` is absent from `ResolvedParaProps` (only `page_break_before`
//! is present). The natural hook is in `flow_paragraph` (this file, currently
//! line 95) immediately after `place_paragraph_layout(state, &resolved, …)`.
//! Adding it is a 4-line change: add `page_break_after: bool` to
//! `ResolvedParaProps` (para.rs), forward from `ParaProps` in `map_para_props`
//! (resolve.rs), and add after `place_paragraph_layout`:
//! ```text
//! if resolved.page_break_after && state.mode.is_paginated() {
//!     finish_page(state);
//! }
//! ```

mod chain;
mod flow;
mod place;

pub(crate) use chain::flow_keep_with_next_chain;
pub(crate) use flow::flow_paragraph;
