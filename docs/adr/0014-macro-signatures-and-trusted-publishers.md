# ADR-0014: Macro signature verification & trusted publishers (Phase 8, Track A)

**Status:** Proposed — the design addendum the macro spec requires *before* any
implementation.
**Date:** 2026-07-20
**Deciders:** AppThere engineering (awaiting ratification)
**Resolves:** [`LOKI_MACRO_SCRIPTING_SPEC.md`](LOKI_MACRO_SCRIPTING_SPEC.md) §2.5
("Signed macros / trusted publishers — deferred (phase 8)") and §14 Phase 8,
which state that signature verification *"requires its own spec addendum before
implementation."* This ADR is that addendum. **No code lands until this is
Accepted.**

---

## 1. Context

The macro subsystem (Phases 1–7) is complete: preserve-first storage, a
sandboxed interpreter, source-only viewing/editing, a per-document trust store
keyed by payload hash, a closed capability catalog, and a gated runner. Trust
today is **per document**: the user enables a specific document's macros, keyed
by the content hash of its macro payload (spec §2.4). There is no notion of
trusting an *author*.

Real corpora sign macros. Office supports a **trusted-publisher** tier: a macro
project signed by a certificate the user has added to their trusted-publisher
store runs without the per-document enable click. This ADR designs that tier for
Loki. Signature *parts* are already preserved opaquely (spec §3.2); this ADR adds
*verification* and a *trust decision* on top — it never changes the bytes.

The spec (§2.5) flags the cost up front: signature verification is *"a large,
security-critical surface (X.509 chains, timestamping, legacy digest agility)."*
The design below is deliberately conservative to keep that surface small.

## 2. Non-goals

- **Re-signing.** Loki never creates or repairs a signature. Editing the body or
  the macros (Phase 7) invalidates any package/macro signature; we surface that,
  we do not re-sign (spec §3.2).
- **Full PKI-to-CA-root trust.** We do **not** grant trust to "anything chaining
  to a public CA root." That model is what makes signed malware effective. Trust
  is anchored on a **user-pinned publisher certificate** (§4.3).
- **Online revocation in this track.** CRL/OCSP require network, which is Track B
  (ADR-0015) and refused until then. Revocation handling is deferred and called
  out as residual risk (§7).
- **XLM / p-code / anything on the "never" list.** Unchanged. A signature over a
  stomped project does not resurrect p-code execution — we still only run
  decompressed source (T5).

## 3. Threat-model delta

Signatures interact with the existing threats (spec §2 table). The dominant one:

- **T10 (trust-metadata forgery) is the whole game.** A document's embedded
  certificate is attacker-controlled data. Verifying that "the bytes are signed
  by the cert embedded in the bytes" proves **nothing** — an attacker signs their
  own malware with their own cert. Therefore: **an embedded signature grants
  trust only when its signer certificate matches an entry the user explicitly
  added to their local trusted-publisher store.** Verification confirms
  *integrity + authorship*; the local pin supplies *trust*. This mirrors §2.4's
  rule that trust lives only in the local profile, never in the file.
- **T5 (stomping).** The signature is computed over the source (VBA: the
  normalized source per MS-OVBA content hash; ODF: the module XML). We verify the
  signature against the *source we will actually run*, so a source/p-code mismatch
  either fails verification or is irrelevant (we never run p-code).
- **T9 (parser exploitation).** Signature/certificate parsing is attacker-facing
  and runs *before* any trust decision, so it must be `forbid(unsafe_code)`, pure
  Rust, fuzzed, and total (malformed → typed `Invalid`, never panic) — the same
  bar as the existing CFB/OVBA/XML readers.

New failure modes to design against: **downgrade** (force the weak legacy digest,
§4.2), **cross-doc signature transplant** (a valid signature blob copied onto
different content — defeated because we verify the signature covers *this*
content), and **expired/rolled-over certs** (§4.4).

## 4. Decision

### 4.1 Verification lives in a new isolated crate, `loki-macro-sig`

A focused crate that takes preserved signature bytes + the source/content they
should cover and returns a typed `SignatureVerdict`. It owns the crypto
dependencies (`cms`/`rasn` for PKCS#7 SignedData, `x509-cert`, `rsa` + `p256`/
`p384` for RSA/ECDSA, `sha2`; `xml-dsig`-equivalent for ODF). `#![forbid(unsafe_code)]`,
no I/O, fuzzed. `loki-macro-host` consumes the verdict; `loki-basic` never sees it
(dependency direction unchanged; servers/headless never link it).

```
enum SignatureVerdict {
    Unsigned,
    Invalid(Reason),                    // malformed, digest mismatch, broken chain
    ValidUntrusted { signer: CertInfo },// integrity OK, signer not in the local store
    ValidTrusted   { signer: CertInfo, thumbprint: [u8;32] },
}
```

Only `ValidTrusted` can raise trust; everything else is display-only.

### 4.2 Digest agility — legacy signatures never grant trust

VBA carries up to three signatures (MS-OVBA / MS-OSHARED): the **legacy**
(`\x05DigitalSignature`, MD5-based), **agile** (`\x05DigitalSignatureAgile`), and
**V3** (`\x05DigitalSignatureV3`) streams. ODF uses W3C XMLDSig in
`META-INF/macrosignatures.xml` / `documentsignatures.xml`.

- The **legacy MD5 signature is never honored for trust** — MD5 is broken; a
  legacy-only project reads as `ValidUntrusted` at best (displayed "signed with a
  legacy method Loki will not trust"). This closes the downgrade attack: presence
  of a strong signature is required, and a strong one cannot be spoofed by
  substituting a legacy one.
- Only **SHA-2-family agile / V3** (VBA) and **SHA-256+ XMLDSig** (ODF) with
  RSA-2048+/ECDSA-P256+ are eligible for `ValidTrusted`.

### 4.3 Trust anchor = user-pinned publisher, not CA chain

A new per-user **`TrustedPublisherStore`** (sibling of the existing per-document
`TrustStore`, same JSON-in-profile pattern, same "nothing in a document can
write it" rule) holds `{ thumbprint: [u8;32], display_name, added: u64 }` entries.
A signature is `ValidTrusted` iff its signer cert's SHA-256 thumbprint is in that
store. The chain **may** be validated for *display* ("issued by …"), but trust is
the pin, not the chain. Adding a publisher is an explicit, anti-spoof-framed user
action (§5), typically "Trust this publisher" from a `ValidUntrusted` document.

### 4.4 Expiry & timestamps, no online revocation (this track)

- A signature whose signing cert is **within validity** verifies normally.
- A signature over a cert that has since **expired** is honored **only** if it
  carries an RFC-3161 **trusted timestamp** proving it was signed while valid;
  otherwise it degrades to `ValidUntrusted` ("publisher certificate expired").
- **Revocation is out of scope for Track A** (needs network — ADR-0015). Until
  then, un-pinning a compromised publisher in the local store is the revocation
  mechanism, and this limitation is stated in the UI and §7.

### 4.5 New trust provenance + decision flow

`Provenance` (ADR from Phase 7.4: `{External, AuthoredHere}`) gains
**`TrustedPublisher { thumbprint }`**. On document open:

1. Read the preserved signature (already in the payload). Verify via
   `loki-macro-sig` against the source the runner would execute.
2. `ValidTrusted` → `MacroService` reports the document **enabled at open**
   without the per-document click, provenance `TrustedPublisher`, **but every
   sensitive capability still prompts** (§2.5: signing proves origin, not
   safety; `DocWrite`/`Clipboard`/`Print`/`FileRead/Write` gating is unchanged,
   `Network` stays refused until ADR-0015). Auto-run-on-open (§5.6) still needs
   its own separate opt-in — a trusted publisher does **not** imply auto-run.
3. `ValidUntrusted` → normal disabled-by-default flow **plus** a "signed by
   \<CN\> — Trust this publisher?" affordance.
4. `Invalid`/`Unsigned` → exactly today's behaviour.

The per-document `TrustStore` and the publisher store are **independent**: a
document can be trusted per-hash *and/or* publisher-trusted; forgetting one does
not touch the other.

### 4.6 Interaction with the macro editor (Phase 7)

Editing a module rewrites the payload → the embedded signature no longer matches
→ verification returns `Invalid`/`Unsigned`. The editor **must warn before the
first edit of a signed project** ("Editing removes \<publisher\>'s signature; the
document will fall back to per-document trust") and, on save, the document
transitions to `AuthoredHere` (Phase 7.4 `reauthor`), never silently re-signed.
This composes cleanly with the existing re-key path.

## 5. UI

- The macros infobar/security panel shows signature state:
  🔏 *"Signed by \<CN\> (trusted publisher)"* / *"Signed by \<CN\> — not trusted"*
  / *"Signature invalid — treated as unsigned"* / unsigned.
- "Trust this publisher…" and the trusted-publisher management list (add/remove,
  with thumbprint + issuer shown) render in the **anti-spoof badged frame**
  (spec §5.5, T7) — never ordinary chrome — because granting publisher trust is
  exactly the target of a spoofing attack.
- All strings via `fl!()` in a new `macros-sig-*` group.

## 6. Implementation phases (once Accepted)

| 8A.n | Deliverable |
|---|---|
| 8A.1 | `loki-macro-sig` crate skeleton + `SignatureVerdict`/`CertInfo` model; workspace + gate registration; `forbid(unsafe_code)`. |
| 8A.2 | VBA signature parsing (MS-OVBA/MS-OSHARED DigSig streams; legacy/agile/V3 discrimination) — **parse only**, fuzzed, total. |
| 8A.3 | PKCS#7 SignedData + X.509 verification; digest/signature-algo agility; content-hash check against the source. |
| 8A.4 | ODF XMLDSig verification (`macrosignatures.xml`). |
| 8A.5 | `TrustedPublisherStore` (persist, T10 tests) + `Provenance::TrustedPublisher`; `MacroService` open-time verdict → decision wiring. |
| 8A.6 | RFC-3161 timestamp handling + expiry policy. |
| 8A.7 | UI: signature state, "Trust this publisher", management list (anti-spoof frame, i18n). |
| 8A.8 | Editor interaction warning; end-to-end tests + fuzz + fidelity-status. |

## 7. Consequences & residual risk

- **Positive:** signed macros from a user-pinned publisher run without the
  per-document click, matching Office ergonomics, while trust stays anchored in
  the local profile (T10 preserved) and capability gating is unchanged.
- **Residual (accepted, documented):** no online revocation in this track — a
  compromised-but-not-yet-un-pinned publisher is trusted until the user removes
  it (mitigated by: pin is per-leaf-thumbprint not per-CA, so the blast radius is
  one publisher; revocation arrives with ADR-0015). Signature verification adds a
  vetted crypto dependency surface — contained in one `forbid(unsafe)` crate,
  fuzzed, and run before any trust decision.
- **New crate, new per-user store** — additive; no change to existing trust,
  capability, or interpreter behaviour when a document is unsigned.

## 8. Open decisions to ratify

1. **Trust anchor (§4.3):** user-pinned leaf thumbprint only (recommended), or
   also allow "trust issuer" (broader, riskier)?
2. **Chain display (§4.3):** validate the full chain for *display* even though
   trust is the pin, or show only the leaf?
3. **Crypto stack:** `cms` + `x509-cert` + `rsa`/`p256` (RustCrypto) vs an
   alternative — pick the vetted set and pin versions.
4. **Timestamp requirement (§4.4):** honor expired-but-timestamped signatures
   (recommended, matches Office), or refuse all expired?
5. **Scope of ODF signatures:** `macrosignatures.xml` only, or also treat a
   whole-document signature as covering macros?
