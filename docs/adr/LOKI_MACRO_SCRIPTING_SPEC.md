<!-- SPDX-License-Identifier: Apache-2.0 -->

# AppThere Loki — Safe Macro Scripting Spec (VBA & StarBasic)

**Status:** Ratified (v1, 2026-07-16) — open decisions D1–D5 accepted as recommended
**Series:** AppThere Client, ADRs M001–M012
**Companions:** ADR-0002 (version-preserving round-trip), ADR-0009 (target
layering), `LOKI_HEADLESS_SERVER_SPEC.md` (C021–C028 — headless policy §10)
**Target edition:** Rust 2024

---

## 0. Scope

Real-world documents contain macros: VBA projects in OOXML macro-enabled
formats (`.docm`, `.xlsm`, `.pptm`, `.dotm`, `.xltm`) and StarBasic/Basic
script libraries in ODF packages (`Basic/`, `Scripts/`,
`<office:scripts>` event bindings). This spec defines how Loki supports
those documents **without becoming a malware vector** — macros are the
single most abused office-document feature in the wild, and every design
decision below starts from that fact.

### Prime directive

> **Security beats compatibility.** Where a VBA/StarBasic feature cannot be
> implemented safely, we break compatibility — deliberately, visibly, and
> permanently. Loki does not aim to run every real-world macro; it aims to
> run the *benign* majority (formatting helpers, data entry, UDFs, mail-merge
> style automation) while making the malicious minority *structurally
> impossible*, not merely prompted-away.

### Goals

1. **Stop destroying macros** (today Loki silently strips them on save —
   see §3). Preserve macro payloads byte-for-byte across load→edit→save.
2. Execute a curated, capability-gated subset of VBA and StarBasic in a
   pure-Rust, `forbid(unsafe_code)`, tree-walking **interpreter** (no JIT —
   iOS-compatible by construction).
3. **Disabled by default** for any document the user did not author, with
   an explicit, per-document, revocable trust grant.
4. **Capability permissions** for anything beyond reading the open
   document: document writes, dialogs, clipboard, file access, printing —
   each individually granted, deniable, and auditable.
5. A **"never" list** (§7) of features that are refused permanently
   regardless of grants: process spawning, FFI, COM/ActiveX, registry,
   p-code execution, Excel 4.0 macro sheets, and others.

### Non-goals

- **Bug-for-bug Office/LibreOffice compatibility.** We implement the
  documented language (MS-VBAL; OOo Basic grammar), not quirks.
- **UserForms / MS-OFORMS rendering** (v1). Deferred, not refused — see §14.
- **A general UNO bridge** for StarBasic. Refused; a small compat shim maps
  the most common idioms onto our object model (§6.4).
- **Macro translation across formats.** Converting `.docm` → `.odt` does not
  transpile VBA to StarBasic (or vice versa); payloads are dropped with a
  warning (§3.5).
- **Server-side execution.** Macros never run in `loki-server` or
  `loki-headless`, ever (§10).

### Engineering standards

Inherited from the workspace conventions: 300-line file ceiling,
`#![forbid(unsafe_code)]` in every new crate, `thiserror` typed errors, no
`unwrap()`/`expect()` in library code, SPDX line 1, `fl!()` for all
user-visible strings, audit-first / implement-second.

---

## 1. Threat model

What macro malware actually does, and which layer of this design stops it:

| # | Attack class | Real-world example | Stopped by |
|---|---|---|---|
| T1 | **Auto-execution on open** | `AutoOpen`/`Document_Open`/`Workbook_Open` droppers; ODF `office:scripts` `OnLoad` event listeners | §2: macros off by default; §5.6: on-open events need a *separate* grant even in trusted docs |
| T2 | **Payload download + execution** | `XMLHTTP` fetch → `Shell`/`CreateObject("WScript.Shell")` | §7: no process spawning, no COM, ever; network is deny-by-default and v2-deferred |
| T3 | **Filesystem ransomware / droppers** | `FileSystemObject`, `Open ... For Output`, `Kill` | §5: file I/O only through OS picker-mediated handles; no path-addressed ambient FS API |
| T4 | **Data exfiltration** | read doc/clipboard → POST to attacker host | §5: clipboard + network are separate capabilities; network deferred to v2 with per-host prompts |
| T5 | **VBA stomping / p-code abuse** | source stream wiped, malicious compiled p-code executes | §4.4: Loki *only* parses decompressed source; p-code and `PerformanceCache` are never read, never executed |
| T6 | **Excel 4.0 (XLM) macro sheets** | `=EXEC()` in hidden macro sheets | §7: never implemented; sheets preserved as inert data, flagged in UI |
| T7 | **Dialog spoofing / social engineering** | fake "security update" MsgBox chains | §5.5: macro-originated dialogs are rate-limited and rendered in a visually distinct, badged frame that app chrome never uses |
| T8 | **Resource exhaustion (DoS)** | infinite loops, gigabyte string concat | §8: fuel metering, memory caps, watchdog + always-available cancel |
| T9 | **Parser exploitation** | malformed CFB/OVBA/XML crafted to exploit the *reader* | §12: parsing is `forbid(unsafe_code)` pure Rust, fuzzed in CI, and runs before any trust decision — so it must be hardened regardless |
| T10 | **Trust-metadata forgery** | document claims "I am trusted" in its own bytes | §2.4: trust state lives *only* in the local user profile, keyed by payload hash; nothing inside the file can influence trust |
| T11 | **Remote/template macro injection** | `.docx` pointing at remote `.dotm` with macros | §7: attached/remote templates are never fetched; template macros only run from a file the user explicitly opened |
| T12 | **Cross-document worming** | macro copies itself into other open docs / Normal.dotm | §6: the object model exposes *only the host document*; no `Documents` collection write-access, no template store, no `VBProject` self-modification API |

Residual risk we accept and document: a user can explicitly trust a
malicious document and grant it document-write access, damaging *that
document* (undo + on-disk original mitigate) — the grants UI is designed to
make the blast radius legible before consent.

---

## 2. M001 — Trust model: authored-by-me, else disabled

### 2.1 Default state

A document containing a macro payload opens with macros **disabled** —
parsed for display purposes at most, never executed. Opening is never
blocked; there is no modal prompt on open (prompt fatigue trains users to
click "Enable"). Instead a passive, non-modal infobar states that macros
are present and disabled (§9.1).

### 2.2 What "the user authored" means

Trust is *never* inferred from the file's own content or metadata (T10).
A document is treated as self-authored only when the **local trust store**
(§2.4) says so:

- A document **created in Loki** on this machine gets a trust-store entry
  at creation time. If the user later adds macros to it *via Loki's macro
  editor* (later phase), those macros are self-authored and may run without
  the enable step (capability prompts still apply).
- A document that **arrives from anywhere else** (file manager, download,
  email, sync folder, collaboration server) has no entry and is untrusted —
  even if its metadata claims the user as author.
- Any **externally-made modification** to a trusted document's macro
  payload (hash mismatch, §2.4) drops it back to untrusted.

### 2.3 The enable flow

From the infobar (or File ▸ Document Security), the user can choose:

| Choice | Effect |
|---|---|
| **Keep disabled** (default) | Payload preserved; nothing executes. Sticky — the infobar collapses to a status-bar chip on subsequent opens. |
| **Enable for this session** | Trust until the document is closed. Not persisted. |
| **Trust this document** | Persistent trust-store entry bound to the macro-payload hash. Re-prompted if the payload changes. |

Enabling **only** permits execution of explicitly-invoked macros with the
baseline capability set (§5.2). It does **not** grant on-open auto-run
(§5.6) or any sensitive capability — those are separate decisions.

### 2.4 The trust store

A per-user, local, versioned store (same app-data directory family as the
spell-checker dictionary cache), **outside every document**:

```
TrustRecord {
  doc_key:        Sha256,        // content hash of the *macro payload* (canonicalised)
  origin_path:    Option<PathBuf>, // advisory display only, never used for matching
  decision:       Disabled | SessionOnly | Trusted,
  auto_run_open:  bool,          // §5.6 — separate opt-in
  capability_grants: Vec<(Capability, GrantScope)>,   // §5.4
  created / last_used timestamps,
}
```

- Keyed by the **hash of the macro payload**, not the file path: renaming
  or copying a trusted file keeps trust; *changing the macros* revokes it.
- Nothing in the store is written into the document; nothing in the
  document is read into a trust decision.
- A management UI lists all records with one-click revocation (§9.4).
- The store is advisory data about *local* decisions; it does not sync via
  the collaboration server in v1.

### 2.5 Signed macros / trusted publishers — deferred

VBA project signatures (MS-OSHARED) and ODF macro signatures could support
a "trusted publisher" tier later. **Deferred** (phase 8): signature
verification is a large, security-critical surface (X.509 chains,
timestamping, legacy digest agility) and the per-document model above is
sufficient for v1. Signature parts are preserved opaquely (consistent with
`loki-opc`'s existing signature policy).

---

## 3. M002 — Storage: preserve first, byte-for-byte

### 3.1 Today's behaviour is data loss

The OOXML importer (`docx/import_package.rs`) walks only known relationship
types and `assemble_docx_kind` builds a **fresh** package on export, so
`vbaProject.bin` / `vbaData.xml` are silently destroyed on save. The ODF
reader (`OdfPackage::open`) extracts a fixed part list; `Basic/`,
`Scripts/`, and `<office:scripts>` are dropped the same way. Fixing this is
**Phase 1** and is valuable even if execution never ships: Loki must stop
corrupting other people's documents.

### 3.2 The macro payload lives in the provenance layer

Following ADR-0002 (`DocumentSource` carries provenance, not document
content), macro payloads attach to `DocumentSource`, **not** to the
document body and **not** to the Loro CRDT:

```rust
// loki-doc-model — provenance layer
pub struct MacroPayload {
    pub kind: MacroPayloadKind,          // OoxmlVba | OdfBasic
    pub parts: Vec<PreservedPart>,       // name, media type, raw bytes
    pub event_bindings: Vec<RawEventBinding>, // detected, for UI/warning only
    pub payload_hash: Sha256,            // trust-store key (§2.4)
}
```

- **OOXML:** `word/vbaProject.bin` (CFB), `word/vbaData.xml`, their
  relationship entries, and the content-type overrides (and the
  `xl/`-rooted equivalents for XLSX). Preserved verbatim; re-emitted on
  export of a macro-enabled kind.
- **ODF:** the `Basic/` and `Scripts/` subtrees, their manifest entries,
  the `<office:scripts>` element (including `script:event-listener`
  bindings), and `Configurations2/` where it references scripts.
- Digital-signature parts remain opaque and untouched, per the existing
  `loki-opc` policy. (Editing the document body invalidates a package
  signature regardless; we do not attempt re-signing.)

### 3.3 Format kinds and extensions

`DocxKind` gains `MacroEnabledDocument` / `MacroEnabledTemplate` (content
types `...document.macroEnabled.main+xml` etc.); XLSX gains the `.xlsm` /
`.xltm` equivalents. Extension ↔ payload consistency is enforced at save:

- Saving a macro-payload document to a **macro-enabled** extension →
  payload re-emitted verbatim.
- Saving to a **macro-free** extension (`.docx`, `.xlsx`, `.odt` chosen
  explicitly by the user) → payload stripped, with a save-dialog notice
  (matches Office behaviour, prevents extension spoofing where a `.docx`
  smuggles a VBA part).
- ODF has no extension split; presence of `Basic/` is governed by the
  payload alone.

### 3.4 Macro editing and write-back (later phase)

When the macro **editor** ships (phase 7), edited VBA modules are written
back **source-only**: the `PerformanceCache`/p-code streams are omitted and
`_VBA_PROJECT` is emitted with the minimal documented header, forcing
Office to recompile from source. This is exactly what LibreOffice does; it
is also a security feature (an edited project can never carry stale
malicious p-code — T5). Until then, payloads are never modified.

### 3.5 Conversion policy

`loki-convert` (headless) and in-app "save as other format" **drop** macro
payloads on cross-family conversion (`.docm` → `.odt`, `.ods` → `.xlsx`,
…) and emit a typed warning (`ConversionWarning::MacrosDropped`). No
transpilation. Same-family conversions that can carry the payload do so.

---

## 4. M003 — Execution engine: one interpreter, two dialects

### 4.1 `loki-basic`: a pure tree-walking interpreter

A new foundation-layer crate implementing lexer → parser → AST →
**tree-walking interpreter**. No JIT, no codegen, no `unsafe`, no
dependencies on I/O of any kind:

- **iOS:** compliant by construction — interpretation only, no runtime
  code generation, satisfying the no-JIT constraint. (See §11 for the
  App Store *policy* dimension, which is separate from the technical one.)
- **Determinism & auditability:** a tree-walker is slow but simple; for
  the macro workloads we target (document automation, UDFs) it is more
  than fast enough, and simplicity is a security property here.
- The interpreter is **resumable/suspendable**: execution proceeds by
  explicit fuel-metered steps (§8) and can block on a host decision
  (permission prompt) or be cancelled between any two steps.

### 4.2 Two dialect front-ends, one core

VBA (MS-VBAL) and StarBasic are near-siblings. One AST and evaluator, with
a `Dialect` flag governing the divergences (default `Option Base`,
`ByRef`/`ByVal` defaults, `Option Compatible` semantics, string/date
coercion quirks, dialect-specific built-ins). Language surface for v1:

- **Types:** `Variant` (dynamic core), Integer/Long/Single/Double/Boolean/
  String/Date/Object/arrays (static + dynamic, `ReDim [Preserve]`),
  user-defined `Type` records, `Enum`, `Const`.
- **Procedures:** `Sub`/`Function`/`Property Get/Let/Set`, optional/named
  arguments, `ParamArray`, modules + (phase 6) class modules.
- **Control flow:** full set (`If`/`Select Case`/`For`/`For Each`/
  `Do`/`While`/`GoTo` within a procedure, `Exit`, `With`).
- **Error handling:** `On Error Resume Next` / `GoTo label`, `Err` object,
  `Error`/`Raise`.
- **Built-ins:** string, math, date/time, conversion, array, and
  `Format`-family functions — the pure-compute standard library.
  Anything that touches the outside world is *not* a built-in; it is a
  host capability (§5) or refused (§7).

### 4.3 Host interface: the interpreter has no authority

`loki-basic` defines a single trait boundary:

```rust
pub trait HostObject { /* late-bound property/method dispatch */ }
pub trait Host {
    fn root(&self, name: &str) -> Option<HostRef>;      // Application, ThisComponent…
    fn request(&mut self, req: HostRequest) -> HostReply; // dialogs, files, everything
    fn consume_fuel(&mut self, units: u64) -> FuelVerdict;
}
```

The interpreter can evaluate expressions and mutate its own heap; **every**
observable effect goes through `Host::request`. A `loki-basic` embedded
with an empty host is a pure calculator. This is the capability seam: the
broker (§5) *is* the `Host` implementation, and nothing the language does
can bypass it — there is no ambient global, no intrinsic I/O function, no
escape hatch to add one from script.

### 4.4 VBA container reading: source only, ever

A new `loki-vba` crate parses `vbaProject.bin`: CFB (compound file) walk →
`dir` stream → per-module MS-OVBA decompression → **source text** (MBCS,
transcoded via the project code page). Hard rules:

- The **p-code / `PerformanceCache` / `_VBA_PROJECT` compiled streams are
  never parsed and never executed** (T5 — VBA stomping). A stomped module
  (empty source, live p-code) is treated as an *empty module*, and the
  mismatch heuristic (module count/offsets vs. source presence) surfaces a
  "project appears tampered" warning in the UI.
- `SRP` streams, designer/OFORMS streams: ignored in v1 (preserved as
  bytes in the payload, invisible to execution).
- The parser is fuzzed (§12) and returns typed errors; a malformed project
  degrades to "macros unreadable — preserved but cannot be enabled".

StarBasic sources are plain XML text inside the ODF package
(`Basic/*/…​.xml`, `script-lb.xml`/`script-lc.xml` library manifests) and
are parsed by `loki-odf` with the existing hardened `quick-xml` stack.

---

## 5. M004 — Capability system: deny by default, grant by decision

### 5.1 Model

Every effectful operation is mapped to a **capability**. The broker (the
`Host` implementation in `loki-macro-host`) checks each `HostRequest`
against the grant table; a missing grant either raises a **prompt** (first
use, §5.4) or returns a **typed denial** the script sees as a trappable
BASIC runtime error (so well-written macros degrade gracefully).

### 5.2 Capability catalog

| Capability | Contents | Default when doc enabled | Notes |
|---|---|---|---|
| `DocRead` | read host document model, selection, metadata | **granted** | the baseline that makes macros useful |
| `DocWrite` | mutate the *host* document via the object model | prompt | all writes batched into CRDT transactions → one undo entry per run (§6.2) |
| `UiDialog` | `MsgBox`, `InputBox`, status text | prompt | badged + rate-limited (§5.5) |
| `Clipboard` | read / write system clipboard | prompt (separate read vs write) | classic exfil/injection channel |
| `FileRead` | read a file **chosen by the user through the OS picker** | picker == consent | no path-string API; see §5.3 |
| `FileWrite` | write to a picker-chosen target | picker == consent | ditto; no overwrite-without-picker |
| `Print` | submit the document to the print flow | prompt | uses the existing print path |
| `Network` | outbound HTTP(S) fetch | **refused in v1** | v2 at earliest, per-host prompts, no raw sockets — see §14 |

Everything not in this table is **refused** (§7). The catalog is a closed
enum in code; adding a capability is a spec-level change, not a patch.

### 5.3 File access is picker-mediated, never path-addressed

The single biggest compat break in the capability design: `Open "C:\…"`,
`FileSystemObject`, `Dir()`, `Kill`, `Name`, `MkDir` **do not exist**.
Scripts that need a file call the object-model equivalents
(`Application.OpenFileForReading(filter…)` shim), which raise the OS file
picker; the user's pick *is* the grant, scoped to that handle, for that
run. This eliminates T3 structurally — a macro cannot enumerate, address,
or touch anything the user didn't hand it — and matches the platform
sandboxing direction on iOS/Android anyway (where the vendored
`loki-file-access` URI-permission patches already work this way).

### 5.4 Grant scopes and prompting

Prompts are asked **at first use during a run** (the interpreter suspends;
§4.1), not as an up-front wall — users decide with the macro's purpose in
view. Each prompt offers:

- **Deny** (default button) → trappable error to the script.
- **Allow once** — this run only.
- **Allow for this session** — until the document closes.
- **Always for this document** — persisted to the trust record (§2.4),
  listed and revocable in the management UI.

There is deliberately **no "always for all documents"** scope.

### 5.5 Anti-spoofing for macro UI

Macro-originated dialogs (T7) render inside a visually distinct frame:
a "Macro: <project name>" badge header in a reserved accent style that app
chrome never uses, with the host document title. Dialog storms are
rate-limited (token bucket, e.g. 5 dialogs / 10 s; exceeding it suspends
the macro with a "misbehaving macro" infobar offering Stop). `MsgBox`
button results are returned normally so benign flows work.

### 5.6 Auto-run events are a separate, scarier decision

Even for a **trusted** document, on-open/auto events (`AutoOpen`,
`AutoExec`, `Document_Open`, `Workbook_Open`, ODF `OnLoad`/`OnStartApp`
listeners, `Auto_Open` in sheets) do **not** fire unless the trust record
has `auto_run_open = true`, set only via an explicit, separately-worded
opt-in ("Run this document's macros automatically when it opens —
recommended only for documents you created"). Explicit invocation (Tools ▸
Macros ▸ Run, assigned buttons) is the normal path. On-close/on-save
events follow the same flag. This single rule neutralises T1, the vector
behind essentially all macro malware campaigns.

---

## 6. M005 — Object model bridge

### 6.1 Facades over the neutral model

`loki-macro-host` exposes per-app object models as `HostObject` facades
over the existing neutral models — **not** over app internals:

- **Text (`loki-text`):** `Application`, `ActiveDocument` → `Document`,
  `Range`, `Selection`, `Paragraphs`, `Characters`, `Find` (phase 6),
  basic formatting properties mapping onto `ParaProps`/`CharProps`.
- **Spreadsheet:** `Application`, `ActiveWorkbook`/`ThisWorkbook`,
  `Worksheets`, `Range`/`Cells` (`Value`, `Formula`, `NumberFormat`),
  `Names`. UDF entry point for cell formulas (§6.3).
- **Presentation:** deferred until the app matures (phase 6+).

The facades expose **only the host document** (T12): there is no writable
`Documents`/`Workbooks` collection over other open tabs, no template
object, no `VBProject`/`VBE` self-modification API, no `Application.Run`
across documents.

### 6.2 Writes are CRDT transactions

All `DocWrite` mutations funnel through the same Loro mutation path the
editor uses (ADR-0006), batched so **one macro run = one undo entry**.
This gives rollback-by-undo for free, keeps collaboration coherent (a
macro edit is an ordinary local edit), and means a runaway-but-permitted
macro is recoverable with ⌘Z.

### 6.3 Spreadsheet UDFs run compute-only

A user-defined function referenced from a cell formula executes with
**zero** capabilities — not even `DocRead` beyond its arguments; no
prompts are possible during recalc. A UDF that attempts any `HostRequest`
returns `#MACRO!`. Tight per-call fuel (§8). This keeps recalculation
pure, fast, and unpromptable.

### 6.4 StarBasic / UNO shim

No general UNO bridge (`createUnoService` is refused — it is the StarBasic
equivalent of COM). A thin shim maps the *common benign idioms* onto the
same facades: `ThisComponent` → active document, `ThisComponent.getText()`
/ text-cursor enumeration, sheet `getCellByPosition`-family, and
`com.sun.star.awt.MessageBox`-style alerts → `UiDialog`. The shim's
surface is an explicit allowlist that grows by demand, never by default.

---

## 7. M006 — The "never" list (permanent compatibility breaks)

The following are **refused unconditionally** — no capability, no prompt,
no configuration flag can enable them. Each raises a distinct, documented
runtime error (`ErrFeatureRefused`, with the feature named) so authors
understand the failure. Preserved payloads may *contain* them; they simply
never execute.

| Refused | VBA / StarBasic surface | Why |
|---|---|---|
| Process execution | `Shell`, `WScript.Shell`, `Environ$` write, `SendKeys` | the dropper endgame (T2) |
| FFI | `Declare Function … Lib`, `DllCall` | arbitrary native code |
| COM / OLE automation | `CreateObject`, `GetObject`, `New` on external ProgIDs, ActiveX | unbounded external surface (T2, T4) |
| UNO service manager | `createUnoService`, `createUnoStruct` (beyond the §6.4 shim) | same, StarBasic flavour |
| Path-addressed file I/O | `Open…For`, `FileSystemObject`, `Dir`, `Kill`, `Name`, `MkDir`, `RmDir`, `FileCopy`, `SetAttr` | replaced by picker-mediated handles (§5.3, T3) |
| Registry / OS settings | `GetSetting`/`SaveSetting`, `RegRead`… | persistence & recon |
| p-code execution | `_VBA_PROJECT`/`PerformanceCache` streams | undocumented, stomping vector (T5) |
| Excel 4.0 XLM macros | macro sheets, `=EXEC()` etc. | legacy pure-malware surface (T6); sheets render as inert data with a warning chip |
| DDE | `DDEInitiate`… | legacy exec vector |
| Remote/attached template code | template macros auto-loaded via `attachedTemplate` URLs | remote macro injection (T11) |
| Timer-based background execution | `Application.OnTime`, `Wait`-loop scheduling | macros run only in a user-visible, cancellable session (§8) |
| Add-in / startup-path loading | global template & add-in directories | nothing executes that didn't arrive in the opened document |
| VBE self-modification | `VBProject`, `CodeModule` object model | self-rewriting malware (T12) |

This table is normative: the interpreter and broker ship with tests
asserting each row raises `ErrFeatureRefused` (§12).

---

## 8. M007 — Resource limits

- **Fuel metering:** every AST step consumes fuel; a run gets a default
  budget (config constant, order 10⁸ steps) — exhausting it suspends with
  a "macro is taking a long time — Continue / Stop" infobar. UDFs get a
  much smaller fixed budget with **no** continue option.
- **Memory caps:** interpreter heap (strings, arrays, objects) accounted
  and capped (order 256 MiB); exceeding → runtime error, not OOM.
- **Recursion/depth caps** and per-run **wall-clock watchdog**.
- **Threading:** macros execute on a worker thread; the UI thread renders
  progress and the always-available **Stop** control. Document mutation
  batches apply via the normal signal path on the UI thread.
- **No sleep/background scheduling** (§7) — a macro is always a foreground,
  user-attributable activity with a visible stop affordance.

---

## 9. M008 — UI/UX

All strings via `loki_i18n` — new domain `macros.ftl` (registered in
`DOMAINS`, per the loader convention). Interactive elements meet the
44×44 px touch-target rule.

1. **Infobar** (new `appthere-ui` component, `AtInfobar`): non-modal strip
   under the ribbon — "This document contains macros. Macros are disabled."
   with `[Enable options…]` opening the trust dialog (an `AtConfirmDialog`
   derivative with the three §2.3 choices, wired like the existing
   `loki_spell::Consent` gate). Collapses to a status-bar `notice_chip`
   ("⚠ macros disabled") on later opens.
2. **Permission prompts** (§5.4): capability name, plain-language
   consequence line, macro + document identity, Deny as default button.
3. **Macro runner:** Tools ▸ Macros — list projects/modules/procedures,
   Run, per-run status line, Stop.
4. **Document Security panel:** per-document trust state, granted
   capabilities with revoke buttons, auto-run toggle (§5.6), "forget this
   document", and the global trust-store list.
5. **Tamper warning** when the VBA project fails the stomping heuristic
   (§4.4).
6. **Macro viewer** (read-only source view, phase 3) — visibility before
   executability: users (and reviewers) can inspect what a macro does
   before enabling anything.

---

## 10. M009 — Server & headless policy

- `loki-server`, `loki-server-collab`, `loki-headless`, `loki-convert`,
  `loki-print`: macro payloads are **opaque bytes**. Preserved through
  storage/collab; **never parsed beyond presence detection, never
  executed**. There is no server-side interpreter dependency at all —
  enforced by keeping `loki-basic`/`loki-macro-host` out of every server
  crate's dependency graph (extend `scripts/check-dependency-direction.py`
  with a denial edge, §12).
- Headless conversion applies §3.5 (preserve within family, strip with
  warning across families).
- Collaboration: the payload rides the document container/provenance
  layer, not the Loro op stream, in v1. Trust remains local per user
  (§2.4) — a collaborator's "trusted" never propagates.

---

## 11. M010 — Platform notes (iOS foremost)

- **Technical:** the engine is a pure interpreter (§4.1); there is no JIT
  anywhere in the design, so iOS's W^X / no-JIT constraint is satisfied by
  construction, with a single codebase for all platforms (no
  interpreter-vs-JIT split to maintain).
- **Policy:** App Store Guideline 2.5.2 restricts executing downloaded
  code; document macros are exactly that. The execution engine is
  therefore behind a **build-time feature flag** (`macro-exec`): the iOS
  build can ship *preservation + viewer only* (still a major win — no
  data loss, full transparency) if App Review requires, without forking
  the codebase. Android/desktop ship with execution enabled.
- Fuel-metered stepping (§8) doubles as the mobile ANR guard.

---

## 12. M011 — Crate layout, and M012 — verification

### New crates (ADR-0009 layer map additions)

| Crate | Layer | Deps (internal) | Responsibility |
|---|---|---|---|
| `loki-basic` | L1 | `loki-primitives` | lexer/parser/AST/interpreter, `Host` trait, fuel. Zero I/O deps; `#![forbid(unsafe_code)]`. |
| `loki-vba` | L2 | — (external: a pure-Rust `cfb` reader) | `vbaProject.bin` CFB walk, MS-OVBA decompression, dir-stream parse, source extraction, stomping heuristic. |
| `loki-macro-host` | L5 | `loki-basic`, `loki-doc-model`, `loki-sheet-model` | capability broker (`Host` impl), trust store, object-model facades, `MacroService` (provided via `provide_context`, `SpellService` pattern). |

StarBasic container parsing lives in `loki-odf`; payload preservation
touches `loki-opc` consumers (`loki-ooxml`, `loki-odf`) and
`loki-doc-model::io::DocumentSource`. UI components land in `appthere-ui`
(`AtInfobar`, permission dialog) and per-app wiring in the three apps.

### Verification (CI-gated)

- **Fuzzing:** `cargo-fuzz` targets for the CFB/OVBA reader, the ODF
  script-container reader, and the `loki-basic` lexer/parser. Corpus
  seeded from real-world macro documents (benign) and CVE-shaped
  malformed containers. Run in CI on a schedule.
- **"Never" table tests:** one test per §7 row asserting
  `ErrFeatureRefused`.
- **Malware-pattern regression corpus:** sanitised auto-open dropper
  skeletons (no live payloads) asserting: not executed on open; T5 stomped
  project treated as empty; XLM sheets inert.
- **Capability tests:** every `HostRequest` kind × {no grant → prompt or
  typed denial; grant scopes honoured; revocation immediate}.
- **Round-trip goldens:** `.docm`/`.xlsm`/ODT-with-Basic load→save byte
  comparison of preserved parts (`loki-acid` fixtures).
- **Dependency gates:** `check-dependency-direction.py` extended: server
  crates must not depend on `loki-basic`/`loki-macro-host`; `loki-basic`
  must not depend on any I/O-capable crate.
- Interpreter conformance suite: language-semantics tests shared across
  both dialects (numeric coercion, error handling, `Variant` edge cases).

---

## 13. Data-loss note on today's behaviour (why Phase 1 is urgent)

Independent of everything above: **Loki currently strips macros from every
macro-enabled document a user saves**, silently. Even users who never want
macro *execution* are having their files damaged. Phase 1 (preservation +
"macros present" indicator) is a correctness fix and should land ahead of,
and independent from, any execution work.

---

## 14. Implementation phases

Each phase is independently shippable and independently reviewable; later
phases can be dropped or reordered without stranding earlier ones.

| Phase | Deliverable | Key crates | Exit criteria |
|---|---|---|---|
| **1. Preserve & detect** | `MacroPayload` on `DocumentSource`; OOXML + ODF payload preservation; macro-enabled `DocxKind`s; extension-strip rule (§3.3); conversion warnings (§3.5); infobar/chip "macros present (not executed)" | `loki-opc` consumers, `loki-doc-model`, `appthere-ui` | round-trip goldens byte-identical; no execution surface exists |
| **2. Interpreter core** | `loki-basic`: full language §4.2, empty-host mode, fuel, suspension; conformance suite; parser fuzzing | `loki-basic` | passes conformance suite; fuzzers clean; zero I/O deps enforced |
| **3. Source extraction & viewer** | `loki-vba` (source-only, stomping heuristic); ODF Basic reader; read-only macro viewer UI | `loki-vba`, `loki-odf`, apps | real-world corpus parses or degrades typed; tamper warning works |
| **4. Trust & capability infrastructure** | trust store, capability broker, permission prompts, Document Security panel, anti-spoof dialog frame | `loki-macro-host`, `appthere-ui` | capability test matrix green; T10 tests green |
| **5. Execution v1 — text + spreadsheet** | object-model facades (§6.1), `DocRead`/`DocWrite`/`UiDialog`/`Clipboard`/`Print`, explicit run only, CRDT-batched undo, Stop control | `loki-macro-host`, apps | "never" table tests green; malware corpus inert; macro run = 1 undo entry |
| **6. Events & UDFs** | button/control-assigned macros; spreadsheet UDFs (compute-only, `#MACRO!`); on-open events behind `auto_run_open` (§5.6); `Find`, class modules | same | T1 regression corpus: nothing fires without the flag |
| **7. Macro editor** | edit + save-back (source-only write, §3.4) for self-authored docs; picker-mediated `FileRead`/`FileWrite` | `loki-vba`, apps | edited projects reopen in Office/LO from source |
| **8. Extended trust (optional)** | signature verification / trusted publishers (§2.5); `Network` capability with per-host prompts — **each requires its own spec addendum before implementation** | new | — |

---

## 15. Open decisions — resolved (2026-07-16)

All five decisions were accepted as recommended:

| # | Decision | Resolution |
|---|---|---|
| D1 | iOS ships execution? | **Accepted:** full engine behind the `macro-exec` build flag; attempt App Review with execution enabled, fall back to preserve+viewer-only if required (§11) |
| D2 | `Network` capability | **Accepted:** refused in v1 (§5.2); v2 at earliest, and only with its own spec addendum |
| D3 | UserForms | **Accepted:** deferred native-widget subset (phase ≥8), not v1 |
| D4 | Trust-store sync across user's devices | **Accepted:** local-only in v1 |
| D5 | Presentation-app object model | **Accepted:** deferred until the app matures; payload preservation still covers presentation-family scripts where the formats are supported |

---

*Once this spec is approved, Phase 1 begins with the preservation work in
§3 — which is a data-integrity fix worth landing regardless of the
execution roadmap.*
