---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: 795
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The prover's bundle is checked against a pinned digest

Raised 2026-11-17 (UTC) by lane `audit/2026-11-17-the-verifier`, as finding 1 of the security audit
[the verifier](../../audit-reports/2026-11-17-the-verifier.md).

Reuse: the digest checks the tree already makes for `cargo-mutants` (`.cargo-mutants-sha256`) and for
Node (`site/.node-sha256`); `kani-verifier`'s own `setup --use-local-bundle`, which installs from a
tarball the caller has already fetched. No new dependency.

Name: provisional. Every new file name below is the lane's, not a ruling.

## The problem

`.kani-version` pins the `kani-verifier` wrapper crate. `cargo kani setup` then downloads
`kani-<version>-<target>.tar.gz` from the model-checking/kani GitHub release and unpacks it with no
digest or signature check. That bundle is the Kani compiler, CBMC and its solvers: everything that
decides whether a proof holds. Both releases the tree uses report `immutable: false`, so the
publisher can replace an asset under its name, and nothing records which bytes a run used.

`script/verify-riscv64` fetches its 0.67.0 bundle the same way, and builds its patched Kani from
`git clone --depth 1 --branch kani-0.67.0`, a tag, with no commit check. It deletes `.git` after
the clone.

## The work

1. A file beside `.kani-version`, provisionally `.kani-sha256`, holds one line per host target the
   tree installs on (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`):
   the target and the SHA-256 of that target's bundle.
2. `script/kani-install` downloads the bundle itself, compares it with the pinned digest, refuses a
   mismatch or a host with no row, and only then runs `cargo kani setup --use-local-bundle <file>`.
3. `script/kani-bump` writes both files in one commit, taking each digest from the release's asset
   metadata, and the bump pull request's body lists the digests it pinned.
4. `script/verify-riscv64` pins its bundle by digest the same way and its source by a full commit
   (provisionally a second line in the patch's header), and checks `git rev-parse HEAD` after the
   clone before it applies the patch.
5. `script/lint` refuses a `.kani-version` without a matching digest row for every target, as it
   refuses a stray install today.

## What this does not do

A digest copied at bump time pins the bytes the bump saw. It narrows a replaced asset to the day of
the bump and makes the bytes a run used a fact in the tree; it does not judge a hostile release. The
queue's full re-falsification on every bump stays the check on that.

## Done when

- A bundle whose digest differs from the pin fails `script/kani-install` before `tar` unpacks it,
  shown by a test that feeds it a modified tarball.
- The riscv64 row refuses a source whose commit differs from the pin.
- A bump pull request changes the version and the digests together, and `script/lint` refuses one
  that changes only the version.

## Index row

Every Kani install checks the bundle, and the riscv64 build checks its source, against a pin before either runs.
