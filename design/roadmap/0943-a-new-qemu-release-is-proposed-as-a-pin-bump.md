---
status: NOT-STARTED
raised: 2026-10-24
milestone_dependencies: none
decision_dependencies: 81
machine_requirements: none
specific_machine: none
needs_person: no
---
# 943. A new QEMU release is proposed as a pin bump, the way a nightly is

*Name: provisional. The number 943 is provisional until the merge queue lands it, and the title
and slug are too.*

calef approved raising this on 2026-10-24 (UTC). Raised only: nothing is built yet.

## Index row

Every gate and benchmark runs under the QEMU `.qemu-version` pins, and nothing says when a newer
release exists. A daily job proposes it as one pull request, the way the nightly bump does, and a
person re-records the floors it moves.

## The gap

`.qemu-version` pins the emulator every gate and benchmark runs under. §81 (dependency upgrades)
calls it a reproducibility pin that a commit raises on purpose, because the benchmarks are icount
instruction counts and a different QEMU legitimately moves them. Nothing tells anyone that a
raise is available. `.github/workflows/toolchain-bump.yml` proposes a Rust nightly every day, and
no job does the same for QEMU. A new stable release, and the fixes in it, reach this tree only when
somebody happens to look.

That matters now. `notes/qemu.md`'s `BUGS` records that QEMU does not order an aarch64 `STLR`
before a later `LDAR`, measured by `tools/host_reordering_litmus` (PR #2668). A fix is posted to
qemu-devel ("Fix RCsc store->load ordering of release stores", 2026-08-21) and is in no release.
When it ships, the tree should hear about it from a job, not from a person rereading a mailing
list.

Reuse: `.github/workflows/toolchain-bump.yml`'s propose step and its fixed-branch pull request,
and `script/ci-qemu`'s build; Renovate and Dependabot were considered and track no QEMU tag.

## What to build

A scheduled workflow, or a job in the existing bump workflow, that follows the same three steps as
the nightly bump: propose, verify, accept.

- Propose: Once a day, read QEMU's release tags. When a stable release newer than
  `.qemu-version` exists, raise the pin on one fixed branch and open or update one pull request.
  Release candidates are ignored. Silent when there is nothing to do.
- Verify: The pull request runs ordinary CI. `script/ci-qemu` builds the new version on a
  cache miss, because the cache key already hashes `.qemu-version`.
- Accept: The icount floors move with the emulator, so a person re-records them on that
  branch with `script/bench --save --why`, reading what moved. Whether the nightly bump's
  `restamp` proof can carry floors across a QEMU bump is an open question below. The default is to
  merge, held only for the compelling reasons `toolchain-bump.yml` lists.
- Never apply: The job must never push to `main`, for the reason `toolchain-bump.yml` gives:
  an unattended job that changes the tool every gate runs under is the authority it should not
  have.
- Say what the release fixes for us: The pull request body lists the `BUGS` entries in
  `notes/qemu.md` that name an upstream fix, so a reviewer can rerun the instrument behind each.
  For the `STLR`/`LDAR` entry that is `tools/host_reordering_litmus/measure.sh`.

## Done when

- A stable QEMU release newer than the pin produces one pull request within a day, with the pin
  raised and the floors left for a person.
- With nothing newer, the job opens nothing and stays green.
- The job's own falsification: pointing it at a pin one release behind opens the pull request.

## Open questions

- One workflow for both pins, or a second one? Sharing `toolchain-bump.yml` keeps one pattern in
  one place; a second keeps a QEMU failure from hiding a nightly one.
- Can the floors be restamped rather than re-saved? A new QEMU can change icount for reasons that
  are not this kernel's, so the nightly's "every row moved under 0.5%" proof may never pass. If it
  cannot, a person re-records every bump.
- Should a point release (11.1.2 to 11.1.3) and a minor release (11.1 to 11.2) be treated alike?

## Follow-on

- **Recorded.** When the `STLR`/`LDAR` fix ships in a release this job proposes, rerun the litmus
  SB test and close the `notes/qemu.md` entry if it reads zero.
