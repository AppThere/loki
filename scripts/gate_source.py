#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Shared source-scanning helpers for the structural gates.

Why this module exists
----------------------
**A source-scanning gate that does not exclude prose will fire on the
documentation of the thing it polices.** Three instances, each fixed separately
before the pattern was named:

  1. The suppression ratchet (Spec 08 §8) tripped on a *comment about*
     `let _ =`.
  2. `check-pending-questions.py` fired on its own `TODO` comments naming the
     instruments they said were missing, plus an OOXML schema and a spike
     write-up — three arrivals, none of them built.
  3. The popover host-purity test matched `place(` inside **its own failure
     message**.

The shape is structural rather than unlucky: the text that describes a forbidden
construct contains that construct, and good documentation contains *more* of it
than bad. So a gate's own explanation is its most likely false positive, and the
better the prose the worse it gets.

One remedy each time, so it belongs here rather than in a fourth place: strip
line comments and string literals before matching.

What this deliberately does **not** do
-------------------------------------
It is not a parser. Block comments spanning lines, raw strings with hashes and
nested quotes are all out of scope, and a gate needing those needs a real
tokeniser instead. The claim is narrower and enough for every case above: after
`code_only`, a mention inside `// …`, `# …` or `"…"` no longer matches.
"""

from __future__ import annotations

import re

# A line whose first non-space characters open a comment, in any of the syntaxes
# the gates scan: Rust/JS, Python/shell/TOML, a continued block comment, XML.
#
# **`#` is a comment only when it is not a Rust attribute.** The first version of
# this helper matched a bare `#`, which made every `#[allow(…)]` and `#[expect(…)]`
# line "prose" and dropped the suppression ratchet's allow count from 188 to
# **zero** — a shared helper silently zeroing a whole metric across every gate
# that adopted it. Caught by measuring the before/after totals rather than by the
# gate itself, which reported the collapse as debt cheerfully resolved.
#
# The lesson is narrow and worth keeping at the regex: one comment syntax is not
# a property of "source", it is a property of a *language*, and a helper shared
# across languages has to say which it means.
COMMENT_LINE = re.compile(r"^\s*(//|\#(?![!\[])|\*|<!--)")

# A double-quoted string literal, non-greedy, honouring backslash escapes. Rust
# and Python both, which is all the gates read.
STRING_LITERAL = re.compile(r'"(?:[^"\\]|\\.)*"')


def code_only(text: str) -> str:
    """`text` with comment lines dropped and string literals blanked.

    Line structure is preserved so a caller can still report line numbers: a
    dropped comment becomes an empty line rather than vanishing.
    """
    out = []
    for line in text.splitlines():
        if COMMENT_LINE.match(line):
            out.append("")
            continue
        # Trailing comments after code, e.g. `let x = 1; // mentions let _ =`.
        stripped = STRING_LITERAL.sub('""', line)
        # Trailing comments after code. `#` again only where it cannot be a Rust
        # attribute.
        for marker in ("//",):
            idx = stripped.find(marker)
            if idx != -1:
                stripped = stripped[:idx]
        hash_at = re.search(r"\s\#(?![!\[])", stripped)
        if hash_at:
            stripped = stripped[: hash_at.start()]
        out.append(stripped)
    return "\n".join(out)


def is_prose_line(line: str) -> bool:
    """Whether `line` is a comment line, for gates that scan line by line."""
    return bool(COMMENT_LINE.match(line))
