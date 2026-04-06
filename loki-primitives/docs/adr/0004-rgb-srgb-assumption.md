# 0004. RGB sRGB Assumption

## Context
`appthere_color::RgbColor` documents its components as being in `[0.0, 1.0]` but does not specify the transfer function (sRGB vs linear). `ThemeColor::apply_tint` and `DocumentColor::resolve_rgb` need to treat `RgbColor` as having a specific encoding for correct behavior.

## Decision
Treat `RgbColor` as sRGB-encoded (gamma-corrected) in all `loki-primitives` operations, with this assumption documented at every call site.

## Rationale
Document formats (ODF, OOXML) store colors as sRGB hex values; the ICC pipeline in `appthere-color` uses profile-relative values, but at the document model layer before ICC transform is applied, sRGB is the correct assumption. The `apply_tint` implementation must linearise before interpolating.

## Consequences
If `appthere-color` clarifies that `RgbColor` is linear in a future version, the `apply_tint` implementation needs updating. This ADR exists so that update is not missed.

## Alternatives rejected
- Treating as linear RGB (would produce visually incorrect tints for document colors stored as sRGB hex)
