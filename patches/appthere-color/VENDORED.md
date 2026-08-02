# appthere-color — vendored copy

**Upstream version:** `0.1.1`, from crates.io
(`registry+https://github.com/rust-lang/crates.io-index`).

**Fork point:** the published `0.1.1` sources, unmodified.

Every file present is **byte-identical** to the crates.io tarball. Two
deliberate differences, and no others:

- `VENDORED.md` and `.gitattributes` are ours; upstream has neither.
- `Cargo.lock` was deleted. A vendored path dependency takes the workspace's
  lockfile, so keeping its own would be a second source for one fact.

`.gitattributes` marks the tree `-text` so git does not normalise line endings.
That is not fussiness: it rewrote `LICENSE` and `README.md` on the way in, and
the removal condition below rests on *"still present with no local
modifications"* — which has to be a question you can answer, and stops being one
as soon as some files differ for reasons that are not modifications.

Check the fork has not drifted:

```sh
C=~/.cargo/registry/src/*/appthere-color-0.1.1
for f in $(cd $C && find . -type f | sed 's|^\./||'); do
  cmp -s "$C/$f" "patches/appthere-color/$f" || echo "differs: $f"
done
# expect exactly: differs: Cargo.lock
```

**Vendored:** 2026-08-02, Spec 08 T5.1 / D-09.

## Why this is vendored, and why it is not a patch

Every other entry under `[patch.crates-io]` exists because upstream has a defect
this suite cannot ship around, and each carries a removal condition naming the
upstream fix. **This one has no defect and no removal condition of that kind.**

`appthere-color` is AppThere's own crate. Phase 5's colour work changes it and
its consumers together, and without vendoring each change is a crates.io release
cycle before the picker can use it. The vendoring is about the *edit loop*, not
about a bug.

That distinction matters for how it should be treated: a patch is a liability to
be removed, and this is a working copy to be published from. **Un-vendor when
Phase 5's colour work is finished and the crate's API has settled** — at which
point the changes are released as `0.1.2+` and this entry is deleted. If the
entry is still here with no local modifications, it has outlived its reason.

## House-standard compliance, measured before deciding to change anything

T5.1 says to measure before committing to fix. Measured at the fork point:

| standard | result |
| --- | --- |
| `#![forbid(unsafe_code)]` on the crate root | ✅ present (`lib.rs:57`) |
| `.unwrap()` / `.expect()` in library code | ✅ **none** — every occurrence is inside a `#[cfg(test)]` module or a doc example |
| typed errors via `thiserror`, no `anyhow` | ✅ `error.rs` derives `thiserror::Error` |
| `license` field matches the suite | ✅ `Apache-2.0` |
| 300-line file ceiling | ✗ one file: `src/policy.rs` at **310** (10 over) |
| SPDX identifier on line 1 of every `.rs` | ✗ **absent on all 11 source files** |

## What was changed as a result: nothing

**The headers are not a defect.** Measuring a crate against a rule it never
adopted and calling the gap a failure is how a house standard becomes an
imposition. If the suite wants SPDX headers across everything it vendors, that is
an argument to make in `appthere-color` upstream, where it costs one commit and
helps every consumer — not in a fork, where it would put *every source file* in
permanent divergence, re-applied at each re-vendor and noise in every upstream
diff for as long as the fork lives.

**`policy.rs` is a real violation — of this crate's own hard rule — and the fix
still does not belong here.** A fork that silently splits a file has changed the
module layout of a published crate without the published crate knowing, so the
next release either carries the change or reverts it, and the fork has to
re-decide every time. The place to split `policy.rs` is upstream. It is recorded
here so the next person to open this directory does not have to re-measure.

The suite's gates agree with leaving both alone: `check-file-ceiling.py` and
`check-license-headers.py` exclude `patches/`, on the stated reasoning that
vendored code keeps its own conventions.

**So the divergence budget is reserved for behaviour.** If a future change to
this crate touches `policy.rs` anyway, splitting it is then nearly free and
should be taken.

## Re-vendoring

1. `cargo add appthere-color@<new>` resolves the new version into the registry
   cache; copy `~/.cargo/registry/src/*/appthere-color-<new>/` over this
   directory.
2. Delete the copied `Cargo.lock`.
3. Restore `.gitattributes` and this file, then re-apply any local
   modifications. There are none as of the fork point — run the drift check
   above, and if it still reports only `Cargo.lock`, ask whether the vendoring
   is still earning its place.
4. Rewrite the measurement table above against the new sources rather than
   assuming it still holds — including re-reading the crate's own `CLAUDE.md`,
   since it is the standard half the table is measured against.
5. **Check the patch is actually used.** `warning: Patch ... was not used` is how
   the Dioxus patches were silently dropped by a version bump (see
   `docs/patches.md`), and the check that does not rely on spotting a warning is
   the lockfile: the `appthere-color` entry must have **no `source =` line**,
   which is what marks it as a path dependency rather than the registry copy.
