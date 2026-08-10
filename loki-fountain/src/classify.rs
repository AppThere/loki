// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Line classification: the body source → a flat element sequence. Fountain
//! is context-sensitive (blank lines and ALL-CAPS decide most elements), so
//! classification runs over the whole line list with lookahead rather than
//! per line.

/// One classified screenplay element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Element {
    SceneHeading(String),
    Action(String),
    /// Centered action (`>text<`).
    CenteredAction(String),
    Character(String),
    Parenthetical(String),
    Dialogue(String),
    Transition(String),
    /// `===` — the next emitted paragraph carries a page break.
    PageBreak,
}

/// True when `line` is blank after trimming.
fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// True when the line has letters and every letter is uppercase.
fn is_all_caps(line: &str) -> bool {
    let mut has_letter = false;
    for c in line.chars() {
        if c.is_alphabetic() {
            has_letter = true;
            if c.is_lowercase() {
                return false;
            }
        }
    }
    has_letter
}

/// Scene-heading prefixes (spec: INT, EXT, EST, INT./EXT, INT/EXT, I/E —
/// each followed by a `.` or space).
fn is_scene_heading(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    ["INT./EXT", "INT/EXT", "I/E", "INT", "EXT", "EST"]
        .iter()
        .any(|p| {
            upper.strip_prefix(p).is_some_and(|rest| {
                rest.starts_with('.') || rest.starts_with(' ') || rest.starts_with('-')
            })
        })
}

/// A transition is ALL CAPS ending in `TO:` (surrounded by blanks, which the
/// caller has already established).
fn is_transition(line: &str) -> bool {
    is_all_caps(line) && line.trim_end().ends_with("TO:")
}

/// A character cue: ALL CAPS (extensions like `(V.O.)` allowed after), not a
/// scene heading, preceded by a blank and followed by content. A `TO:` line
/// is *not* excluded here — the transition rule requires a following blank
/// and the cue rule requires following content, so the contexts are already
/// mutually exclusive, and per the spec a caps `TO:` line followed by
/// dialogue is a cue.
fn is_character_cue(line: &str) -> bool {
    let name = line.split('(').next().unwrap_or(line).trim();
    !name.is_empty() && is_all_caps(name) && !is_scene_heading(line)
}

/// Classifies the body lines. Forced markers (`.` `!` `@` `>` `~`) override
/// the context rules, per the spec's power-user section.
pub(crate) fn classify(body: &str) -> Vec<Element> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let raw = lines[i];
        let line = raw.trim();
        if line.is_empty() {
            i += 1;
            continue;
        }
        let prev_blank = i == 0 || is_blank(lines[i - 1]);
        let next_blank = i + 1 >= lines.len() || is_blank(lines[i + 1]);

        // ── Page break ────────────────────────────────────────────────────
        if line.chars().all(|c| c == '=') && line.len() >= 3 {
            out.push(Element::PageBreak);
            i += 1;
            continue;
        }

        // ── Forced elements ───────────────────────────────────────────────
        if let Some(rest) = line.strip_prefix('.')
            && !rest.starts_with('.')
        {
            out.push(Element::SceneHeading(rest.trim().to_string()));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix('!') {
            out.push(Element::Action(rest.to_string()));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix('@') {
            i = dialogue_block(&lines, i, rest.trim(), &mut out);
            continue;
        }
        if let Some(rest) = line.strip_prefix('>') {
            if let Some(centered) = rest.strip_suffix('<') {
                out.push(Element::CenteredAction(centered.trim().to_string()));
            } else {
                out.push(Element::Transition(rest.trim().to_string()));
            }
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix('~') {
            // Lyrics: styled as action (the template has no lyric style).
            out.push(Element::Action(rest.trim().to_string()));
            i += 1;
            continue;
        }

        // ── Context rules ─────────────────────────────────────────────────
        if prev_blank && is_scene_heading(line) {
            out.push(Element::SceneHeading(line.to_string()));
            i += 1;
            continue;
        }
        if prev_blank && next_blank && is_transition(line) {
            out.push(Element::Transition(line.to_string()));
            i += 1;
            continue;
        }
        if prev_blank && !next_blank && is_character_cue(line) {
            i = dialogue_block(&lines, i, line, &mut out);
            continue;
        }

        out.push(Element::Action(line.to_string()));
        i += 1;
    }
    out
}

/// Consumes a character cue at `lines[i]` (already identified; `cue` is the
/// cue text without any forcing marker) plus its dialogue block: subsequent
/// non-blank lines, `(…)` ones as parentheticals. Returns the next index.
fn dialogue_block(lines: &[&str], i: usize, cue: &str, out: &mut Vec<Element>) -> usize {
    out.push(Element::Character(cue.to_string()));
    let mut j = i + 1;
    while j < lines.len() && !is_blank(lines[j]) {
        let line = lines[j].trim();
        if line.starts_with('(') && line.ends_with(')') {
            out.push(Element::Parenthetical(line.to_string()));
        } else {
            out.push(Element::Dialogue(line.to_string()));
        }
        j += 1;
    }
    j
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
