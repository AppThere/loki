#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
#
# Runs the structural gates CI runs, and *blocks the push* if any fail.
#
# Why this exists (Spec 08 ADR L08-035, applied to process rather than to a bench)
# ------------------------------------------------------------------------------
# The file-ceiling gate was run, failed, and its output was read after the push
# rather than before it — so a commit that breaks CI reached the branch while the
# information needed to stop it was already on screen. Remembering to read output
# at the right moment is the same class of control as remembering to warm up a
# bench: it gets skipped exactly when attention is on the change being made.
#
# So the check moves to a moment that cannot be skipped without noticing. These are
# the cheap structural gates only — seconds, no compilation. `cargo clippy` and the
# test suite are deliberately *not* here: a pre-push hook that takes minutes gets
# disabled, and a disabled hook protects nothing.
#
# Install:
#     git config core.hooksPath .githooks
#
# Bypass, when you mean to (a WIP branch, a baseline update in progress):
#     git push --no-verify

set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 1

GATES=(
    check-arc-get-mut.py
    check-dependency-direction.py
    check-file-ceiling.py
    check-license-headers.py
    check-loki-basic-pure.py
    check-no-hardcoded-viewport-dims.py
    check-no-panics.py
    check-retracted-claims.py
    check-suppressions.py
    check-todo-format.py
    check-unsafe-policy.py
)

failed=()
for gate in "${GATES[@]}"; do
    [ -f "scripts/$gate" ] || continue
    if ! output=$(python3 "scripts/$gate" 2>&1); then
        failed+=("$gate")
        printf '%s\n' "$output"
    fi
done

if [ ${#failed[@]} -gt 0 ]; then
    printf '\npre-push: %d structural gate(s) failed: %s\n' \
        "${#failed[@]}" "${failed[*]}" >&2
    printf 'These are the same gates CI runs. Fix them, or push with --no-verify\n' >&2
    printf 'if you are deliberately pushing a work-in-progress branch.\n' >&2
    exit 1
fi

printf 'pre-push: %d structural gates OK\n' "${#GATES[@]}"
