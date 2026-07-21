# ADR-0015: Macro `Network` capability (Phase 8, Track B)

**Status:** Proposed — the design addendum the macro spec requires *before* any
implementation.
**Date:** 2026-07-20
**Deciders:** AppThere engineering (awaiting ratification)
**Resolves:** [`LOKI_MACRO_SCRIPTING_SPEC.md`](LOKI_MACRO_SCRIPTING_SPEC.md) §5.2
(`Network` = *"refused in v1 … v2 at earliest, per-host prompts, no raw
sockets"*), §14 Phase 8, and §15 D2 (`Network` *"v2 at earliest, and only with
its own spec addendum"*). This ADR is that addendum. **No code lands until this
is Accepted.**

---

## 1. Context

`Network` is the one capability the catalog (spec §5.2) defines but **refuses
unconditionally** in v1: `Capability::Network` exists and `is_refused_in_v1()`
returns `true`, so it can never be prompted or granted. It was deferred because
outbound network access re-opens two threats the v1 design deliberately closed:

- **T2 — payload download + execution** (`XMLHTTP` fetch → run it), and
- **T4 — data exfiltration** (read the document/clipboard → POST to an attacker).

This ADR designs a **narrow, opt-in, off-by-default** network capability whose
mitigations keep those threats closed *even with network enabled*, and states the
residual risk that remains.

The load-bearing observation: **the "never" list (spec §7) is not relaxed.**
Process execution, COM/OLE, FFI, and path-addressed file I/O stay refused. So the
classic T2 chain `download → Shell/CreateObject → execute` is still broken at the
*execute* end regardless of this capability — network can fetch bytes, but nothing
in the sandbox can turn fetched bytes into a running process. That is what makes a
bounded network capability defensible.

## 2. Non-goals

- **Raw sockets, arbitrary protocols.** HTTPS request/response only. No TCP/UDP,
  no WebSocket, no `Winsock`, no `MSXML2.XMLHTTP` (COM — stays on the never list).
- **Relaxing the never list.** Unchanged. `Network` does not add `Shell`, COM,
  FFI, registry, or path-addressed FS.
- **Server / headless macro execution.** Servers and the headless CLI never link
  the interpreter (spec workspace policy) and therefore never get this capability.
  The network *effect* is provided through the app-only `MacroBackend`/`Host`
  seam, exactly like picker-mediated `FileRead`/`FileWrite`.
- **Ambient credentials.** No cookies, no OS proxy auth, no client certificates,
  no Kerberos/NTLM — a macro cannot ride the user's existing network identity.

## 3. Threat-model delta

| Threat | Status with a bounded `Network` capability |
|---|---|
| **T2 (download+execute)** | Fetch is possible; **execution is not** — process spawn/COM/FFI remain never-listed, so fetched bytes cannot be run. Residual: a macro could write fetched bytes to a **picker-chosen** file (`FileWrite`) that the *user* later runs manually — mitigated by the file-picker consent (T3) and by prompting for `Network` and `FileWrite` separately. |
| **T4 (exfiltration)** | **Re-opened and accepted as the core residual risk.** `DocRead` (baseline) + `Network` = a macro can POST document content to an allowed host. Mitigations: off by default; per-host prompt with the destination shown; `Deny` default; the anti-spoof frame; and (§4.6) an explicit stronger warning when a document holds *both* `Network` and a read capability. This risk is inherent to *any* network capability and is the reason it stays opt-in and per-host. |
| **T7 (dialog/prompt spoofing)** | The per-host grant prompt is a prime spoofing target → rendered in the badged anti-spoof frame (spec §5.5), rate-limited, host shown verbatim (punycode-decoded with a homograph warning). |
| **T8 (DoS)** | Response size cap, connect/read timeouts, a cap on total requests per run, all under the existing fuel/watchdog; every request is cancellable via the always-available **Stop**. |
| **T9 (parser exploitation)** | Response bodies are attacker-controlled input handed back to macro code as bytes/string only — Loki never parses them into a privileged format. TLS/HTTP parsing is delegated to the vetted `reqwest`/`rustls` stack already used elsewhere in the app. |

## 4. Decision

### 4.1 A gated object-model shim, not a socket API

Scripts reach the network only through object-model verbs on the host facade,
e.g. `Application.HttpGet(url)` and `Application.HttpPost(url, body, contentType)`,
returning an `HttpResponse { status, headers, bytes }`. There is **no**
`CreateObject("MSXML2.XMLHTTP")` (COM, refused) and no socket type. Each call
gates the `Network` capability through the broker before doing anything.

### 4.2 HTTPS-only, per-host allowlist, per-host prompts

- **Scheme:** `https` only. `http`, `ftp`, `file`, and everything else are
  refused (a plaintext or local-file fetch is never allowed).
- **Grant unit = origin** (`scheme + host + port`), never a bare "network on"
  switch. The first request to `https://api.example.com` prompts:
  *"This macro wants to connect to **api.example.com**. Allow?"* with the standard
  `GrantScope` choices (`Deny` default / `AllowOnce` / `AllowSession` /
  `AlwaysForDocument`) — reusing the exact broker + prompt machinery built in
  Phase 4. No wildcards; each distinct origin prompts once.
- **Redirects** are followed only to already-allowed origins; a redirect to a
  new origin re-prompts (or fails if non-interactive). Redirect count capped.

### 4.3 No ambient authority

No cookie jar, no OS/proxy credential reuse, no client certs, no automatic
`Authorization`. The macro may set explicit request headers **except** a
deny-list (`Host`, `Cookie`, and hop-by-hop headers are stripped). This prevents
a macro from silently acting as the signed-in user against a first-party service.

### 4.4 Bounded execution

Enforced by the app-side backend, not the interpreter: connect+read timeouts, a
maximum response size (streamed, hard-capped), a maximum number of network calls
per run, and cancellation wired to the run's existing cancel flag so **Stop**
aborts an in-flight request. Network calls happen on the worker thread (Phase 5
async runner), never blocking the UI.

### 4.5 Where the effect lives (dependency direction)

`loki-basic` stays I/O-free (enforced by `check-dependency-direction`). `Network`
is dispatched like every other effect: the interpreter asks its `Host`, the
`loki-macro-host` `ExecutionHost` gates the capability, and the actual request is
performed by an app-provided `MacroBackend` impl (using the app's existing
`reqwest`/`rustls`). The headless/server backends implement it as *always
refused*, so no server path can make outbound calls via a macro.

### 4.6 Capability composition warning

Because `Network` + a read capability is the exfiltration primitive (T4), the
grant prompt is **context-aware**: if the document already holds `DocRead`
(always) or a `Clipboard`/`FileRead` grant, the `Network` prompt adds an explicit
line — *"This document can also read your content; allowing network access lets a
macro send it to this site."* `AlwaysForDocument` for `Network` requires the
stronger confirmation variant of the anti-spoof dialog.

## 5. Relationship to ADR-0014 (signatures)

- A **trusted-publisher** signature (ADR-0014) does **not** auto-grant `Network` —
  §2.5's rule holds: signing proves origin, not safety. Network always prompts
  per host, even for publisher-trusted documents.
- Once ADR-0014 ships, `Network` unlocks **online revocation** (CRL/OCSP) for
  signatures — a natural follow-on, gated by the same allowlist against the CA's
  published endpoints. This ADR does not depend on 0014; the two compose.

## 6. Implementation phases (once Accepted)

| 8B.n | Deliverable |
|---|---|
| 8B.1 | Flip `Capability::Network` off the `is_refused_in_v1` list **behind an off-by-default build/runtime flag**; broker + `GrantScope` plumbing for origin-scoped grants; tests. |
| 8B.2 | `HttpResponse`/request model + the object-model shim (`Application.HttpGet/HttpPost`) in `loki-macro-host`; interpreter dispatch through the `Host` seam. |
| 8B.3 | App `MacroBackend` network impl: HTTPS-only, origin allowlist, redirect re-check, header deny-list, no ambient creds (`reqwest`/`rustls`). |
| 8B.4 | Bounds: timeouts, size cap, per-run call cap, Stop-cancels-request; worker-thread wiring. |
| 8B.5 | Context-aware per-host prompt + composition warning in the anti-spoof frame; i18n `macros-net-*`. |
| 8B.6 | Headless/server backend = always-refused; dependency-direction + "never links interpreter" tests; fidelity-status. |

## 7. Consequences & residual risk

- **Positive:** macros that legitimately need a bounded HTTPS fetch (data
  refresh, licensed API) work, opt-in and per-host, without opening raw sockets,
  COM, or process execution.
- **Residual (accepted, documented):** **T4 exfiltration is inherently
  re-opened** — a user who grants a document both content-read (baseline) and
  network-to-host-X can have content sent to host X. This cannot be fully
  eliminated by any network capability; it is bounded by off-by-default,
  per-host consent, the composition warning, HTTPS-only, and no ambient
  credentials. This residual is the explicit price of the feature and must be
  ratified as acceptable.
- **T2** stays structurally closed (no execute primitive), which is the reason a
  network capability is defensible at all.

## 8. Open decisions to ratify

1. **Ship gate (§6, 8B.1):** off-by-default *runtime* setting the user must
   enable, a *build* flag (like the iOS `macro-exec` flag), or both?
2. **`AlwaysForDocument` for `Network`:** allow it (with the stronger
   confirmation), or cap network grants at `AllowSession` so they never persist?
3. **`HttpPost` at all**, or ship read-only `HttpGet` first (POST is the sharper
   exfil edge)?
4. **Header policy (§4.3):** exact deny-list, and whether author-set
   `Authorization` is allowed (explicit tokens) or stripped.
5. **iOS/App-Review posture:** does an enabled network capability affect the
   `macro-exec` App-Review strategy (spec §11)?
