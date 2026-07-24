# Macro signatures — corpus gathering checklist (Track A residual gates)

Track A (ADR-0014) shipped its verifier, trust store, and UI, but **four
deliberate gates remain open** because they cannot be closed without a corpus of
**real, third-party-signed documents**. Every existing signature test builds its
own fixture in-process (fresh `rcgen` self-signed RSA / P-256 keys, our own
canonicaliser producing the signed octets) — which proves our code is
self-consistent but *cannot* prove it agrees with Microsoft Word or LibreOffice.
That agreement is the whole point of a signature verifier.

This document is the gathering checklist: what samples are needed, how to
produce them, what each one unblocks, and what "done" looks like.

**Do not implement the blocked items before the corpus exists.** A content-hash
or canonicalisation routine written from the spec alone and validated only
against itself will produce a hash mismatch that is indistinguishable from
tampering — i.e. it will silently report *every* real signed document as
`Invalid`, which is worse than today's honest `Unsigned`.

## 1. What is blocked, and on what

| Gate | Where | Blocked on |
|---|---|---|
| `TODO(8A.3-corpus)` | `loki-macro-sig/src/verify.rs` | Real CMS/PKCS#7 `SignedData` — does RustCrypto `cms`/`x509-cert` parse what Word/LO actually emit? |
| `TODO(8A.8-vba-content)` | `loki-macro-host/src/verify.rs` | The MS-OVBA V3/agile **signed-content hash** normalisation. VBA currently reads `Unsigned` because we cannot compute the hash Word signed. |
| `TODO(8A.4-corpus)` | `loki-macro-sig/src/verify_odf.rs`, `xml_c14n.rs` | LibreOffice `XMLDSig` interop — our inclusive C14N must yield **byte-identical** `SignedInfo` octets to LO's. |
| `TODO(8A.6-tsa-anchor)`, `TODO(8A.6-xades)` | `loki-macro-sig/src/timestamp.rs`, `verify_odf.rs` | RFC-3161-timestamped samples, and ODF `xades:SignatureTimeStamp` samples. |

## 2. The corpus

Each row is one sample. **Record the producing application and its exact
version** alongside every file — a signature quirk is only diagnosable if you
know who wrote it.

### 2.1 VBA (unblocks 8A.3 + 8A.8) — the highest-value samples

Produce in Word: create a `.docm` with a trivial `Sub`, then
*Developer ▸ Visual Basic ▸ Tools ▸ Digital Signature…* and pick a code-signing
certificate. Save, and keep the **unsigned original** too.

- [ ] **V-1** `.docm`, signed, Word 365 (current) — the baseline case.
- [ ] **V-2** `.docm`, signed, an **older Word** (2016/2019) — writes a different
      stream mix; this is where the legacy vs agile split shows up.
- [ ] **V-3** `.xlsm`, signed — confirms the same path works off the Word host.
- [ ] **V-4** The **unsigned original** of V-1 — must read `Unsigned`, not
      `Invalid` (guards the honest-degradation contract).
- [ ] **V-5** V-1 with **one module edited after signing** (open, change a
      comment, save without re-signing) — must read `Invalid`. This is the
      negative that proves the content hash is actually load-bearing; a
      normalisation bug that ignores source content would pass V-1 and fail here.
- [ ] **V-6** Signed with an **ECDSA P-256** code-signing cert, if obtainable —
      exercises the non-RSA branch against a real signer.
- [ ] **V-7** Signed **with a timestamp** (Office timestamps when the signing
      cert's issuer supplies a TSA) — feeds 8A.6.

For each, note which of `_DigitalSignature` / `_DigitalSignatureAgile` /
`_DigitalSignatureV3` streams are present. `loki_macro_sig::extract_vba_signatures`
already locates all three, so this can be dumped with a small `examples/` binary
before any new verification code is written.

### 2.2 ODF (unblocks 8A.4) — the canonicalisation gate

Produce in LibreOffice: *Tools ▸ Macros ▸ Organize Macros* to add a Basic
module, then *Tools ▸ Macros ▸ Digital Signature…* → Sign.

- [ ] **O-1** `.odt` with a signed Basic macro library, current LibreOffice.
- [ ] **O-2** `.ods` with a signed Basic macro library.
- [ ] **O-3** An `.odt` with **both** `macrosignatures.xml` *and*
      `documentsignatures.xml` (sign the document as well) — confirms we read the
      macro signature and ignore the document one.
- [ ] **O-4** O-1 with a **module edited after signing** → must read `Invalid`.
- [ ] **O-5** An `.odt` signed by an **older LibreOffice** (or OpenOffice, if
      available) — namespace-prefix and whitespace habits change between
      versions, and that is exactly what C14N interop is sensitive to.
- [ ] **O-6** If LO can be made to sign a **subset** of modules, capture it: our
      coverage check must reject a signature that leaves an executable module
      unreferenced (ADR-0014 §4.5). If LO cannot produce this, record that as a
      finding — it means the fail-closed reading is untested against a real
      signer and should stay conservative.

### 2.3 Timestamps (unblocks 8A.6)

- [ ] **T-1** A VBA signature with an RFC-3161 token (this may be V-7).
- [ ] **T-2** An ODF signature carrying `xades:SignatureTimeStamp`.
- [ ] **T-3** **Expired signing cert + valid timestamp.** Hard to find in the
      wild and the most valuable timestamp sample, because it is the only one
      that exercises the rescue path. *Produce it deliberately:* issue a
      code-signing cert with a very short validity (e.g. 1 day) from a local CA,
      sign with it, timestamp against a public TSA (freetsa.org, or a commercial
      one), then wait out the validity window. Start this early — it has a
      built-in delay no amount of engineering removes.

## 3. Constraint: what may be committed

Real signed files embed the signer's certificate, which carries a **real name,
organisation, and often an email address**. A signature cannot be redacted —
altering any byte invalidates it, which destroys the sample's value.

So, for each sample, decide up front:

- [ ] **Self-produced samples** (signed with a cert *we* created for this
      purpose, with a deliberately fictitious subject) → **commit** them, under
      `loki-macro-sig/assets/`, loaded with `include_bytes!` following the
      `loki-acid` fixture precedent (`loki-acid/src/fixtures.rs`).
- [ ] **Third-party / vendor-signed samples** (a real published macro-enabled
      document) → **do not commit**. Keep them in a local corpus directory and
      have the test read `LOKI_MACRO_CORPUS=/path/to/corpus`, skipping cleanly
      when unset. CI stays green without the corpus; a developer with it gets the
      stronger check.
- [ ] Confirm redistribution rights before committing **anything** obtained from
      a third party, including the certificate chain.

The self-produced route is strongly preferred and covers every gate except
"does a *stranger's* Word emit something we choke on" — which is what a couple
of read-only, uncommitted vendor samples are for.

## 4. Per-gate unblock plan

Once the corpus exists, in this order:

- [ ] **8A.3** — Run the existing `verify_signed_data` against V-1..V-3, O-1..O-2
      with the content hash supplied by hand (extracted from the sample). Any
      parse failure is the ADR-0014 §6 decision point: **if a real-world
      `SignedData` quirk defeats RustCrypto `cms`, fall back to `rasn-cms`.**
      Record the outcome either way — "RustCrypto handled the corpus" is itself
      the finding that closes this gate.
- [ ] **8A.8** — Implement the MS-OVBA contents-hash normalisation (legacy,
      agile, V3 each differ) in `loki-vba`, validating each against V-1/V-2/V-3
      until the computed digest equals the one in the sample's `signedAttrs`.
      Then flip `MacroPayloadKind::OoxmlVba` in `loki-macro-host/src/verify.rs`
      off `SignatureVerdict::Unsigned`. **V-5 must go `Invalid`** before this is
      considered done.
- [ ] **8A.4** — Feed O-1..O-5 through `verify_xmldsig`. On any mismatch, dump
      our canonical `SignedInfo` octets beside LO's and diff — the failure will
      be in namespace rendering or attribute ordering, not the crypto. Add the
      surviving cases as C14N regression vectors in `xml_c14n_tests.rs`.
- [ ] **8A.6** — With T-1/T-2 in hand, decide whether to anchor the TSA chain.
      Note the current limitation is *sound* under leaf-thumbprint pinning (a
      forged `genTime` cannot make attacker content trusted, because trust still
      requires a pinned publisher), so this is a hardening step, not a bug fix.
      T-3 proves the expired-but-timestamped rescue end-to-end.

## 5. Definition of done

- [ ] Each gate's `TODO(...)` is either **removed** (validated) or **rewritten**
      to state precisely what the corpus showed and why it remains open.
- [ ] Every committed sample has a test asserting its expected verdict, and every
      "must be `Invalid`" negative (V-4 is `Unsigned`, V-5 / O-4 are `Invalid`)
      passes.
- [ ] `docs/fidelity-status.md` §13 loses the corresponding "residual gate" text.
- [ ] The RustCrypto-vs-`rasn-cms` decision is recorded in ADR-0014.

## 6. Status

**Not started — blocked on sample acquisition, which is a human/procurement
task, not an engineering one.** Nothing in §4 should begin until §2 has samples
and §3 has a commit decision for each.
