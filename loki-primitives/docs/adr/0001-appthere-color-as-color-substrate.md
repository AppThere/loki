# 0001. appthere-color as Color Substrate

## Context
We need color value types and color math for document primitives.

## Decision
We decided to depend on `appthere-color` and re-export its types, rather than defining our own types or depending on `palette` directly.

## Rationale
`appthere-color` is already published by the same project, is Apache-2.0 licensed, is ICC-accurate for print workflows, and explicitly states that document-semantic color concepts belong in consuming crates. Adding a dependency on `palette` in addition would create two separate color type hierarchies in the same document pipeline.

## Consequences
`loki-primitives` takes a version dependency on `appthere-color`; breaking changes in `appthere-color`'s color value types require a coordinated version bump here. This is acceptable given both crates are under the same project ownership.

## Alternatives rejected
- `palette` directly (would duplicate types already in `appthere-color`)
- own color value structs (same duplication problem)
- `bevy_color` (game-engine dependency, wrong ecosystem)
