<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# DOCX save allocates 8.5× what it did — bisected, attributed, unfixed

| | |
|---|---|
| Status | **Open** — cause located and measured; no fix attempted |
| Found | 2026-08-16, running `loki-bench` on Windows 11 (RTX 3050 box) |
| Culprit | `05644320` "Add DOCX Word-compatibility repair; fix acid2 fixture and the export path" |
| Instrument | `cargo bench -p loki-bench --bench portable_io` (dhat allocation counts) |

## Observed

Against the committed baseline (`loki-bench/baselines/portable.txt`):

| key | bytes | allocs |
|---|---|---|
| `io/large_save` | +85.9% | **+747.0%** |
| `io/medium_save` | +25.6% | **+631.7%** |
| `io/small_save` | +5.7% | **+359.7%** |
| `io/large_open` | +23.3% | +15.5% |
| `io/medium_open` | +20.4% | +14.8% |
| `io/small_open` | +14.0% | +12.3% |

Bytes up modestly while allocation *counts* explode is the signature of many
small allocations replacing few large ones.

## The baseline is cross-platform comparable — this is a real regression

The obvious objection is that these are Windows numbers against a
Linux-calibrated baseline, and several `io/*` keys depend on the system font
scan. Checked directly, by running the same bench at the baseline commit
(`ae606db8`) on the same Windows machine:

| | Linux baseline | Windows @ `ae606db8` |
|---|---|---|
| small save | 2 186 362 B / **720** | 2 186 331 B / **720** |
| medium save | 2 299 839 B / **1 922** | 2 299 870 B / **1 922** |
| large save | 2 752 812 B / **6 484** | 2 754 402 B / **6 484** |

Within ~30 bytes and **zero allocations**. So the `[REGRESSED]` labels are
trustworthy, and — worth carrying forward — the standing caution that `io/*`
figures might diverge across platforms is **not** borne out for these keys.

## Attribution: the whole regression is one pass

`git bisect` over the 559 commits between `ae606db8` and the list-export work
landed on `05644320`. Its own message names the mechanism: *"The export assembly
now runs the same canonicalisation pass as its final step, so every DOCX Loki
writes is schema-ordered."*

`write/assembly.rs` calls `docx::repair::canonicalize_package(&mut pkg)` on
**every** export. Per WML part it clones the bytes unconditionally
(`repair/mod.rs`), parses the whole XML into a purpose-built DOM, reorders
children into the ECMA-376 sequence, and re-serialises — a second full
parse/serialise cycle per save.

Commenting out that one call returns every figure to within **0.2%** of the
original baseline:

| `io/large_save` | allocs | bytes |
|---|---|---|
| HEAD | 54 918 | 5 116 902 |
| pass disabled | **6 495** | **2 752 140** |
| baseline `ae606db8` | 6 484 | 2 752 812 |

So the pass accounts for 100% of it: 8.5× the allocations, 1.86× the bytes.

**A hypothesis this refutes.** The list-export writers (`dc482103`) were the
obvious suspect — that commit writes full 9-level `w:abstractNum`s and its
writer allocates ~6 short-lived `String`s per level. Measured at `dc482103^`,
the numbers were *already* fully regressed (3 310 / 14 063 / 54 917, matching
HEAD). The list writers contribute essentially nothing.

## Do not simply delete the pass

It is a correctness fix. OOXML complex types are `xsd:sequence`s, so Word
**refuses to open** a `.docx` whose `pPr`/`rPr` children are out of schema
order, while tolerant readers (Loki, LibreOffice) accept any order. The pass is
what stops files Loki saves from tripping Word's repair prompt, and it is locked
by `loki_export_is_word_schema_clean`. This is a deliberate
correctness-for-allocations trade that was simply never measured.

## Two directions, and the trap in the first

1. **Root cause — emit in schema order at the writers**, demoting the pass to a
   debug assertion. Bigger than it looks: `write/style_props.rs:49` carries the
   comment *"canonicalisation pass places these in CT_PPr order, so emit here"*,
   so at least one writer now **depends** on the pass to place its output.
   Every writer would need auditing before the pass could be removed, and
   removing it prematurely reintroduces the Word-compatibility bug.
2. **Cheaper — make the pass allocate less.** `repair_part` already returns
   `None` when nothing changed, but the unconditional `part.bytes.clone()`
   before it runs is pure waste on the common path. That is the obvious byte
   win; the *allocation* count is the DOM itself, which only (1) removes.

## Not established

No profile attributes the allocations within the pass — that it is the DOM
rather than, say, the attribute maps is inference from the shape of the numbers
(counts up far more than bytes), not measurement. Wall-clock cost was not
measured at all: this is an allocation finding, and `portable_io` is an
allocation instrument. Whether the extra allocations are perceptible on a real
save is a separate question that needs a timing bench.
