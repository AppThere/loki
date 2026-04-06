# 0003. `Px` Excluded from Unit Conversion

## Context
`Px` (pixels) cannot be converted to physical units without a DPI value that varies by device and rendering context.

## Decision
Do not implement `UnitConversion` for any pair involving `Px`.

## Rationale
A pixel has no fixed physical size; providing a conversion would silently embed an assumed DPI (typically 96) which is wrong for print, HiDPI displays, and mobile devices; callers must handle DPI explicitly.

## Alternatives rejected
- `UnitConversion<Px, Pt>` with assumed 96 DPI (incorrect for too many real-world cases)
