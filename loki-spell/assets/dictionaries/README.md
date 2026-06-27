# Bundled & cataloged dictionaries

Spell-check dictionaries are standard Hunspell `.aff` (affix rules) + `.dic`
(word list) pairs, sourced from
[`wooorm/dictionaries`](https://github.com/wooorm/dictionaries) (which repackages
upstream Hunspell dictionaries with normalized structure and per-language
licenses). Catalog download URLs pin that repository to an **immutable commit**
and are integrity-checked by SHA-256, so the bytes are reproducible.

## Bundling policy

Only **permissively** licensed dictionaries may be bundled in the application
binary (see `LicenseClass::is_bundleable`). Copyleft (GPL) and lesser-copyleft
(LGPL / MPL) dictionaries are **not** bundled — they are downloaded on demand
after explicit user consent, which preserves the user's right to obtain, modify,
and redistribute them under their own terms.

## Bundled here

| Tag | Language | License | Source |
| --- | --- | --- | --- |
| `en` | English | `(MIT AND BSD)` (SCOWL-derived) | `dictionaries/en/` |

The full upstream license / attribution text for the bundled dictionary lives
beside it in `en/license`. When adding another **bundled** dictionary, copy its
license text in the same way and keep this table and `assets/catalog.json` in
sync.

## Cataloged for download

`assets/catalog.json` additionally lists downloadable dictionaries (currently
French `MPL-2.0`, Spanish `(GPL-3.0 OR LGPL-3.0 OR MPL-1.1)`, German
`(GPL-2.0 OR GPL-3.0)`). Extend that manifest — with real `sha256`/`size` and a
pinned URL — to offer more languages; no code change is required.
