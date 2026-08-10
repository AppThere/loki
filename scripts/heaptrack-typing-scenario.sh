#!/usr/bin/env bash
# The usage-audit §15/A4 discriminating measurement: is the reported memory
# growth *allocator retention* (glibc arenas holding the high-water mark of
# per-keystroke Document clones + layout churn) rather than a live leak?
#
# A4 predicts: heaptrack's "peak heap" ≈ RSS during typing, near-zero leaked
# bytes at exit, and a materially lower RSS under MALLOC_ARENA_MAX=2 for the
# same workload. A live grower (A1/A2) instead shows heap-in-use climbing in
# heaptrack itself — and the loki_text::mem log line (enabled below) says
# which one: para_cache_bytes (A1) or loro_ops with loro_history_cache=true
# (A2). Run this on a desktop with a display; the workload is interactive.
#
# Usage:
#   scripts/heaptrack-typing-scenario.sh            # heaptrack run
#   ARENA_CHECK=1 scripts/heaptrack-typing-scenario.sh   # MALLOC_ARENA_MAX=2 run
#
# Workload (same for both runs, ~5 minutes):
#   1. Open the app, create a blank document (Ctrl+N).
#   2. Type continuously for ~2 minutes (real prose, not one held key).
#   3. Note RSS (the loki_text::mem lines print alongside; `ps -o rss` works).
#   4. Save (Ctrl+S) — watch for the A2 step-down.
#   5. Type ~2 more minutes, close all tabs, note RSS again (A1 predicts it
#      stays high: the paragraph cache lives on the app root).
#   6. Quit normally so heaptrack can classify leaked-vs-reachable.
#
# Compare: heaptrack_print's "peak heap memory consumption" and "total memory
# leaked" against the observed RSS. RSS >> peak heap points at the allocator
# (A4) or off-heap GPU memory (A3 — read the texture_resident/texture_peak
# fields in the same log lines).
set -euo pipefail
cd "$(dirname "$0")/.."

# Release + debug symbols, so heaptrack's stacks resolve without debug-build
# allocation noise (a debug build allocates differently enough to mislead).
export CARGO_PROFILE_RELEASE_DEBUG=true
cargo build --release -p loki-text

export RUST_LOG="${RUST_LOG:-loki_text::mem=info}"

if [[ "${ARENA_CHECK:-0}" == "1" ]]; then
    # The falsification run: glibc arena retention capped. If RSS for the same
    # workload drops substantially versus the plain run, A4 is confirmed and
    # the remedy is allocator guidance, not a leak hunt.
    export MALLOC_ARENA_MAX=2
    echo ">> MALLOC_ARENA_MAX=2 — run the same workload, compare RSS." >&2
    exec target/release/loki-text
fi

command -v heaptrack >/dev/null || {
    echo "heaptrack not installed (e.g. apt install heaptrack heaptrack-gui)" >&2
    exit 1
}
exec heaptrack target/release/loki-text
