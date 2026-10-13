---
status: BUILT
raised: 2026-10-10
built: 2026-10-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 870. `main` keeps its CI caches warm

*(The title, the slug and the `main-caches` job name are provisional.)*

## The defect

A pull request or merge-group run restores `actions/cache` entries from its own ref and from
`refs/heads/main` only. A′ (ci.yml's `gate` job, notes/merge-queue.md) skips the suite on a push to
`main` that a merge group already tested, which is nearly every push, and with it every cache step.
So a new cache key never reached `main`. After #1899 moved `.qemu-version` to 11.1.2, `main` held
only the 11.1.1 entries, every pull request and merge-group QEMU job spent 265 to 274 s in "Build
QEMU if the cache missed", and `test` went from about 13 to 17.4 to 18.5 minutes. calef fixed it by
hand by dispatching ci.yml on `main` (run 38084559582, saves at 20:45 and 20:47 UTC). The BUGS of
milestone 855 (the TLS graph enters the gated build) recorded it. verify.yml's patched Kani for
`prove-kernel-riscv64` had the same gap: A′ skips that job on `main` too.

## What was built

A `main-caches` job in ci.yml, one leg per runner pool (`ubuntu-24.04` and `ubuntu-24.04-arm`),
that runs on every `push` event and on nothing else. The push trigger is `main` only. It does not
read the gate, so a change to A′ cannot reopen the gap. Each leg asks `actions/cache` with
`lookup-only: true` whether `main` holds the key, and on a miss runs the build script the restoring
jobs run (`script/ci-qemu`, and on ARM64 `script/verify-riscv64 --build-only`); the action's post
step then saves under the same key on `refs/heads/main`. Keys and paths are copied from the jobs
that restore them. The same job puts back an entry the cache's eviction removed, at the next push.

| cache | key | legs | restored by |
|---|---|---|---|
| QEMU | `qemu-<os>-<arch>-<hash of .qemu-version>` | X64, ARM64 | X64: `swish-check-x86_64`, `bench`; ARM64: `test`, `swish-check`, `boot-check`, `cpu-matrix-shard`, `watchdog` (ci.yml); scheduled workflows on `main` |
| patched Kani | `kani-riscv64-<os>-<arch>-<patch>-<hash>` | ARM64 | `prove-kernel-riscv64` (verify.yml) |

Not covered, on purpose: `supply-chain`'s vendor tarballs (a miss costs one HTTP request), and
rust-cache, which calef's ruling (a) on #1814 kept on three jobs only, saved outside merge groups
(notes/actions-cache-budget-2026-10-07.md). Nothing else in ci.yml or verify.yml uses
`actions/cache`; the std farm and the toolchains are not cached.

Reuse: bevy's `update-caches.yml` (a workflow on push to `main` builds and saves, and CI jobs
restore), found by notes/actions-cache-budget-2026-10-07.md section 3. Taken as a pattern, as one
job in ci.yml rather than a workflow of its own, because the restoring jobs' keys sit in the same
file and a reader comparing them should not need two.

## How it was proven

The workflow logic, step by step:

- Cold key on a push to `main`: the job's `if:` admits `push`; `lookup-only` finds no entry, so
  `cache-hit` is `false`; the build step runs and fills the path; the post step saves it, scoped to
  the run's ref, `refs/heads/main`.
- Warm key: `lookup-only` finds the entry and downloads nothing; `cache-hit` is `true`; both build
  steps are skipped; nothing is saved.
- Pull request and merge group: the job is skipped by condition and takes no runner.

Exercised on 2026-10-10 (UTC) by run 38088783769, a dispatch on this branch from a temporary
commit that was dropped before review. That commit added `-trial870` to both keys so the trial
could not touch `main`'s entries, admitted `workflow_dispatch`, and had the gate skip the suite.

| attempt | leg | lookups | built | saved | job time |
|---|---|---|---|---|---|
| 1 (cold) | X64 | QEMU missed | QEMU, about 4 minutes | QEMU | 4.4 min |
| 1 (cold) | ARM64 | both missed | QEMU, then Kani in 142 s | both | 6.8 min |
| 2 (warm) | X64 | QEMU found | nothing | nothing | 0.1 min (9 s wall) |
| 2 (warm) | ARM64 | both found | nothing | nothing | 0.1 min (10 s wall) |

The three entries attempt 1 saved were scoped to the branch's ref, which is the ref-scoping a push
to `main` relies on; they were deleted afterwards. With the salt removed, each key matched `main`'s
live entry character for character (the 11.1.2 QEMU keys calef's dispatch saved, and the Kani key
of 2026-10-03), so the job looks up what the restoring jobs restore. Not exercised: a `push` event
on `main` itself.

## BUGS

- ~~A key that drifts from its restoring job's saves an entry nobody reads, and nothing gates
  that.~~ Closed 2026-10-10 (UTC) by #1908 (an owed correction: this entry was true when merged).
  `helpers/cache_save_scope.py`, run by `script/lint`, now fails when a path, key or pinned
  `actions/cache` SHA in `main-caches` differs from any restoring step's in any workflow, or when
  no step restores it at all. What it still does not compare is runners: a restoring job moved to
  a pool `main-caches` has no leg for reads a key `main` never saves.
- When the suite also runs on a push to `main` with a cold key (a commit landed outside the queue),
  this job and the suite's QEMU jobs all build, and one save wins. That costs runner minutes, never
  a result.
- What was exercised before merge was a dispatch on this branch, not a push to `main`. The first
  `.qemu-version` or Kani patch change after merge is the first real cold run on `main`.

## Follow-on

- **Done.** A lint check that each key here equals its restoring job's key, in
  `helpers/cache_save_scope.py` (#1908, 2026-10-10 UTC). It covers paths and the pinned SHA too.
- **Refused.** Warming `supply-chain`'s vendor tarballs: a miss costs one HTTP request, less than
  the job would.

## Index row

When a pinned tool's version moves, pull requests and merge groups stop rebuilding it on every
job the moment the change lands on `main`, rather than when somebody remembers to dispatch CI there.
