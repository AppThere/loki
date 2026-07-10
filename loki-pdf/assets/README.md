<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 AppThere Loki contributors -->

# Bundled PDF assets

## `CGATS001Compat-v2-micro.icc`

A compact CMYK ICC profile embedded by default as the PDF/X `OutputIntent`
`DestOutputProfile` (see `crate::options::OutputIntent`).

- **Source:** [saucecontrol/Compact-ICC-Profiles](https://github.com/saucecontrol/Compact-ICC-Profiles)
  (`profiles/CGATS001Compat-v2-micro.icc`).
- **License:** **CC0 1.0 Universal** (public domain dedication) — see
  [`CC0-1.0.txt`](./CC0-1.0.txt). CC0 places no restrictions on use or
  redistribution, so it is compatible with this crate's Apache-2.0 license.
- **Characterization:** CGATS TR 001-1995 (the U.S. SWOP coated-web reference).
  Data color space **CMYK**, PCS `Lab`.
- **Contents:** the `desc`, `cprt`, `wtpt`, and `A2B0` tags only. `A2B0`
  (CMYK → PCS) is the direction an output intent needs to *characterize* the
  DeviceCMYK content this crate emits; the profile intentionally omits the
  reverse `B2A0` table (it is not used to convert *into* CMYK here).

### Scope note

This is a deliberately small, permissively-licensed default so every export
carries an embedded CMYK characterization instead of merely naming a printing
condition. It is **not** a full press-house profile: for certified press output
(e.g. a specific ISO Coated / FOGRA condition) supply that profile via
`OutputIntent::with_icc_profile`, which overrides this default.
