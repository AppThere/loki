<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike findings

Investigation documents produced by a spec's Phase 0. No production code; each
document is evidence for a decision taken later.

## Loki Spec 09 — Layout Memory

| ID | Document | Answers | Verdict |
| --- | --- | --- | --- |
| S09.0 | [Layout residency census](S09.0-layout-residency-census.md) | Spec 09 §3 Q1–Q7 **and E0** | **45–63% of layout residency is evictable for text-bearing documents; object-heavy content is far higher (98%).** Body text runs 123 B/char, 69 of it evictable, flat across a 25× size change. The *fraction* is the durable number — it moves by under 2× where the rate moves 60×. Eviction is **safe** (layout is a pure function of the CRDT) but not *representable*: `None` already means "read-only", so an evicted page would read as a silently wrong answer. Checkpoint recovery exists but only at clean page tops. Glyph items are stored **three** times — sharing one allocation removes ~26% with no eviction machinery |

**E0 has been run** (S09.0 §10), so Spec 09 L9-005 is satisfied and the phase
plan is unblocked. Spec 09 §4 describes E0 as a manual RSS comparison needing
real hardware; it does not — layout is CPU-only, so it runs headless under dhat,
which also disposes of both methodological caveats §4 raises. It is committed as
`loki-bench/benches/layout_editing_residency.rs` and doubles as the regression
guard for the steps that follow.

The census's headline figure survived contact with the instrument (predicted 72
B/char, measured 70.1, flat across a 4× document-size change). Its *total* did
not, and the 36 B/char gap led to the cheapest win on the list.

Then E0 was pointed at the conformance corpus — and chasing an inconsistency in
the result found that **the instrument was order-dependent**: one-time costs,
font loading above all, were billed to whichever measurement ran first. Warm,
almost every corpus figure changed, by up to **252×**, and the "floor artefact at
4.5k characters" turned out not to exist. Corrected numbers in S09.0 §10a: real
formatting costs 1.7× the synthetic rate (not 2.5×), and the evictable band is
**45–63% for text-bearing documents** with object-heavy content far higher.
Spec 09 should still target the fraction — but note it is a property of
*documents*, not a goal for us; what we control is how much of it we reclaim.

The failed ×10 experiment then paid for itself. Repetition cannot vary size at
constant formatting (it changes the cache-hit profile), but run as a sweep at
×1/×2/×5/×10 it decomposes residency into **~78 B/char keyed to paragraph
content and ~39 B/char paid per placement**, with residuals under 0.04 B/char.
That sizes S9-1 from measurement rather than struct arithmetic, and means
boilerplate-heavy documents deduplicate for free (S09.0 §10b).

**S9-1 has shipped** (S09.0 §10c prediction, §10d result). `ParaCache` now holds
`Arc<ParagraphLayout>`, so the cache and the page editing index share one
allocation: body-text editing residency **69.4 → 34.8 B/char**, total
**123.3 → 89.0** (−27.8%), with `C` unchanged at 78.2 and `P` collapsing
39.3 → 1.1 — exactly the split predicted before the code was written, per
L9-013. The total now lands on the census's *original* ~88 B/char prediction,
which the extra copy had been hiding. The prediction protocol earned its keep
before the measurement did: deriving it found that §10b's account of which
coefficient S9-1 would move was wrong, and corrected it ahead of the work. The
run also caught what the prediction missed — read-only residency rose ~11 B/char,
because the deep `clone` into the cache had been compacting glyph vectors as a
side effect nothing had named.

**S9-2 has shipped too** (§10f prediction, §10g result). `ByteIndexMap` — `u32`
entries plus an `Identity` variant — took body text to **73.0 B/char total,
34.8 editing**, against 124 and 69.4 when E0 first ran: **41% off total
residency across the two steps**, no eviction machinery, no contract change. The
prediction that mattered here was the *null* one: C and P were predicted not to
move, and did not, because after S9-1 the maps live in one place and appear in
both measured conditions. A harness reporting only the duplication sweep would
have called S9-2 a no-op while total residency fell 18%.

**Three follow-ups landed with S9-2's review** (S09.0 §10h, §10i). R9-15 is now
measured rather than recorded: E0 has a CJK tier, and **CJK costs ~3× Latin per
character** (111.7 vs 34.8 B/char editing) while the evictable fraction stays
inside the text-bearing band (50.9% vs 47.7%) — so the rate does not transfer
across scripts and the fraction does, the same split the duplication sweep found
on an independent axis. The tier fails rather than prints if no CJK face resolves,
since tofu shapes into a perfectly believable number. L9-016's `Arc::get_mut` ban
is a CI gate (`scripts/check-arc-get-mut.py`), verified by negative test to fail
on a real call and pass on prose about one. And S9-3's governing metric is
derived as **page-access-set bounds** rather than C and P — which found that the
scan inventory is four sites, not one, and that the worst of them
(`recompute_page_index`) starts at page 0 and runs on *every keystroke*.

Three of those follow-ups then produced results worth having. **R9-16 — the
per-byte hypothesis — is refuted by its own discriminator** (§10j): a
Cyrillic+Greek tier at 1.85 bytes/char reads 88.1 B/char where per-byte predicts
135.1, and per source byte the three scripts read 73.0 / 47.7 / 74.7 rather than
agreeing. The CJK/Latin match was a two-point coincidence. Neither characters nor
bytes are invariant — but the **evictable fraction is, across all three scripts
(47.7 / 50.4 / 50.9%)**, now the third independent axis supporting L9-008. And
**the per-keystroke scan is measured** (§10l): 3.3 µs at 445 pages, 13.6 µs at
889, so it is not a present-day latency defect and S9-3 stays architecture — but
the measurement corrected §10i, because cost is *flat* in caret position, meaning
the loop runs to completion and the access set is the whole document rather than
a prefix.

## Loki Spec 08 — UX & Memory Remediation Program, Phase 0

| ID | Document | Gates | Verdict |
| --- | --- | --- | --- |
| S0.1 | [Blitz scroll capability](S0.1-blitz-scroll-capability.md) | Phases 1, 2, 7 | 5 of 6 capabilities already exist and ship today. R1 closed |
| S0.2 | [Render pipeline and memory census](S0.2-render-memory-census.md) | Phase 2 | Textures are already windowed; the unbounded axis is zoom×DPI, not page count. Phase 2's acceptance criterion needs restating |
| S0.3 | [Coordinate-space audit](S0.3-coordinate-space-audit.md) | T1.3, T3.1, T5.5 | Chain documented; §3.3's stated cause for I-06 is wrong; a better candidate identified |
| S0.4 | [IME patch archaeology](S0.4-ime-patch-archaeology.md) | T3.2 | Patch intact; the **Android build of `loki-text` is broken** by a duplicated entry point. R6 closed |
| S0.5 | [Page style format study](S0.5-page-style-format-study.md) | Phase 6 | §3.4 confirmed; most of T6a/T6b already built under ADR-0012. Phase 6 is much smaller than scoped |
| S0.6 | [Device capability probe](S0.6-device-capability-probe.md) | T1.6 and all later responsive work | 11 behavioural `cfg(target_os)` sites, all enumerated. R13 closed |

### Exit criteria

Spec §4 Phase 0 requires six findings documents and confirmation that the §7
decisions still hold. Both are met. All eight decisions survive the findings
unchanged; four gain a consequence:

| Decision | Status | Consequence from Phase 0 |
| --- | --- | --- |
| D-01 patch locally, PR upstream | **Holds** | Cheaper than assumed — S0.1 finds no new patch is needed for scroll. The one new patch candidate is input-device events for pointer precision (S0.6 §4) |
| D-02 page-style names in a custom part | **Holds** | S0.5 §4 identifies `docx/write/custom_props.rs` as the worked example to copy |
| D-03 OS measurement system → locale → metric | **Holds** | S0.6 §4 folds the probe into `DeviceProfile`'s plumbing |
| D-04 calibrate on first use of Actual Size | **Holds** | S0.6 §4 confirms R7: physical display size is frequently absent or wrong, so calibration is the primary path as D-04 assumes |
| D-05 character-based measure | **Holds** | `reflow_metrics.rs` already has a pixel cap (`MAX_REFLOW_TILE_PX`) to replace |
| D-06 extract `appthere-color-ui` | **Holds, with a correction** | **`appthere-color` does not exist.** The picker lives in `appthere-ui/src/components/color_picker/` (`mod.rs`, `custom.rs`, `convert.rs`). T5.1 must create *both* crates, or restate D-06 as "extract the existing `appthere-ui` picker into a standalone pair". This is the one §7 decision written against a component that is not there |
| D-07 styles document-scoped, defaults app-scoped | **Holds** | Document half already implemented (S0.5 §1); the application-scoped half is new |
| D-08 budget derived per device at runtime | **Holds** | S0.6 confirms the violation set is small; S0.2 §5 finds the largest violation is the Android renderer path, not the budget |

### Recommended changes to the spec before Phase 1 starts

1. **Phase 2 acceptance (§4).** Replace "peak RSS for a 500-page document within
   20% of a 10-page document" with a resident-texture-bytes bound. Texture
   residency is already document-length-independent; the RSS difference between
   those documents is layout and editing data, which Phase 2 as scoped does not
   touch. Add the layout tail as a new issue rather than absorbing it silently.
   (S0.2 §4, §6.)
2. **Add an issue for the broken Android build.** S0.4 finds a compile failure,
   not the behavioural regression I-07 describes. It should be tracked and fixed
   ahead of Phase 3 rather than inside it, since nothing on Android can be
   verified until it builds.
3. **Rescope Phase 6 (§4).** T6a.1, T6a.2, T6a.3, T6b.1, T6b.2, T6b.3 and most
   of T6b.5 are already implemented. The real backlog is `style:page-usage`,
   the page-size catalogue, the EMU migration of the page family only,
   application-scoped defaults, the advisory custom part, and all of T6c.
   (S0.5 §1, §4.)
4. **Correct §3.3.** Spelling squiggles are already emitted per layout line;
   the leading candidate is the fragment clip floor discarding the descender
   band. (S0.3 §4.)
5. **Note in §3.1** that the overlay *and* most scroll capabilities are proven,
   so T1.1 is a documentation task.

### Open items carried out of Phase 0

| Item | Owner phase | Note |
| --- | --- | --- |
| ~~Identify the sixth scroll capability (spec r3 §3.1)~~ | — | **Answered** in S0.1 §2a: the missing one is **animated programmatic scroll**, which is app-side work, so T1.1 has no patch to land. Nested containers are *unproven*, not missing |
| Probe P1 — nested scroll containers | T1.7 (Phase 1) | S0.1 §4. **Still open** — needs a running app; not runnable in the dev sandbox. Test input routing, not layout: blitz-dom models the geometry, so the plausible failure is renders-right/routes-wrong. Gates T7.3; R2 is *unverified*, not unsupported |
| Wire the `DeviceProfile` platform probes | **distributed** — T2.0, T3.2, T4.0, T5.5, T7.1 | The type, context and pointer latch landed in Phase 1 (`appthere-ui/src/device_profile.rs`); every probe behind it is still `Unknown`. Per spec r5 the probes now land **with their consuming phase** rather than as a standalone tail — a probe with no consumer cannot be tested, and the consumer is the first thing that would notice a wrong value. The 11 behavioural `cfg` sites from S0.6 §2a retire as their probe arrives, so L08-011 is asserted but not yet true |
| Screen-test each phase before the next builds on it | every phase | Spec r6 §3.6, now the program's leading risk (R23). The first screen test of Phase 1 found I-20 — a functional regression that had passed 31 unit tests, the full workspace suite, the CI clippy command and eight script gates. Automated gates cannot see behavioural regressions, and unverified phases stack |
| Close Phase 0.5: run the negative test, and make CI reach the branch | next CI run | S0.4 §6b. Half one (host gates are blind) is measured; half two needs an NDK the sandbox cannot fetch. Note the branch currently triggers **no** CI — `rust.yml` fires only on `main` pushes and PRs to `main` |
| Confirm I-06 candidate 1 with a failing test | T3.1 | S0.3 §4 |
| Decide Phase 2 acceptance criterion (a) or (b) | before T2.1 | S0.2 §4 |
| I-21: pick the branch of the T1.9 diagnostic | next screen test | Inspection ruled out the margin arithmetic, chrome inside `client_height`, and a missing zoom factor; two causes remain and a `tracing::debug!` on `loki_text::caret_follow` separates them in one observation. See the `editor_caret_follow` module docs. **The margin value has deliberately not been tuned** — three of T1.9's four causes are bugs that lowering it would mask |
| Reconcile L08-003 with ADR-0012 Decision 2 | Phase 6 ADR pass | S0.5 §7 |
