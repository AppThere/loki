# 0002. Deviation Isolation

## Context
Real-world OOXML files frequently violate the strict specifications codified actively defining explicit boundaries dynamically generating exceptions structurally.

## Decision
All deviation handling is explicitly isolated within the `src/compat/` directories natively mapping exceptions independently gated directly by `#[cfg(not(feature = "strict"))]` compiler configurations carrying explicit citations mapping natively.

## Rationale
Mixing deviation logic globally inside core format parsers invalidates ISO conformance tests rendering specifications mathematically impossible to explicitly verify.

## Consequences
Setting `--features strict` inherently converts `loki-opc` into an authorized conformance standard checking framework reporting explicitly what paths logically trace configurations blocking extraction implicitly testing dependencies cleanly resolving constraints optimally isolating environments transparently exposing structures perfectly tracking paths effectively resolving outputs safely generating structures precisely.
