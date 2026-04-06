# 0002. No `num-traits`

## Context
`Length<U>` arithmetic needs an additive identity (zero).

## Decision
Implement `zero()` as an inherent method rather than depending on `num-traits`.

## Rationale
`num-traits` is a broad dependency; we need only one concept from it; the manual implementation is three lines.

## Consequences
Not interoperable with `num-traits` ecosystem by default; acceptable since `loki-primitives` is not a numeric library.

## Alternatives rejected
- `num-traits` (overkill)
- `zero` crate (unmaintained)
