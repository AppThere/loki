// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Frame `style:graphic-properties` serialisation, split from `auto.rs` for the
//! 300-line ceiling. Called by `AutoStyles::graphic_style` (the parent module).

use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

use super::attr_str;

/// Serialises a floating frame's `<style:graphic-properties/>`: the text-wrap
/// (`style:wrap` + `style:run-through`), a solid fill (`draw:fill` +
/// `draw:fill-color`), and a solid stroke (`draw:stroke` + `svg:stroke-color`).
/// `fill`/`stroke` are bare `RRGGBB` hex (as stored on the text box's attr);
/// a leading `#` is added for the ODF colour value. Returns an empty string when
/// nothing is set.
pub(super) fn emit_graphic_properties(
    wrap: Option<FloatWrap>,
    fill: Option<&str>,
    stroke: Option<&str>,
) -> String {
    let mut s = String::new();
    if let Some(w) = wrap {
        // Mirror of the import `map_graphic_wrap` (frames.rs): the ODF wrap token
        // and, for a run-through/behind float, the run-through position.
        let wrap_val = match (w.wrap, w.side) {
            (TextWrap::None, _) => "run-through",
            (TextWrap::TopAndBottom, _) => "none",
            (_, WrapSide::Left) => "left",
            (_, WrapSide::Right) => "right",
            (_, WrapSide::Largest) => "dynamic",
            _ => "parallel",
        };
        attr_str(&mut s, "style:wrap", wrap_val);
        let run_through = if w.behind_text {
            "background"
        } else {
            "foreground"
        };
        attr_str(&mut s, "style:run-through", run_through);
    }
    if let Some(hex) = fill {
        attr_str(&mut s, "draw:fill", "solid");
        attr_str(&mut s, "draw:fill-color", &with_hash(hex));
    }
    if let Some(hex) = stroke {
        attr_str(&mut s, "draw:stroke", "solid");
        attr_str(&mut s, "svg:stroke-color", &with_hash(hex));
    }
    if s.is_empty() {
        String::new()
    } else {
        format!("<style:graphic-properties{s}/>")
    }
}

/// Prefix a bare `RRGGBB` hex string with `#` for an ODF colour attribute
/// value; leaves an already-`#`-prefixed value unchanged.
fn with_hash(hex: &str) -> String {
    if hex.starts_with('#') {
        hex.to_string()
    } else {
        format!("#{hex}")
    }
}
