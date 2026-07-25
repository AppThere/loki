<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike findings

Investigation documents produced by a spec's Phase 0. No production code; each
document is evidence for a decision taken later.

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
