---
status: PARTIAL
raised: 2026-11-05
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1033. `generational_table`: the fixed-capacity, proved slotmap, released on its own

*(Number provisional until the integrator mints it. The title and slug are drafts. Milestone 820
(`extensible_host_controller_interface`: proven xHCI logic, released on its own) is the shape this
follows.)*

`generational_table` is the third crate in the ruled publication order (see the release order of milestone 857 (crates released on their own)).
The architect reviewed it and ruled publish. The crate is the fixed-capacity,
allocation-free, proved member of the slotmap family. Its README states how it differs and lists its
costs under `BUGS`. It points users who need growth to `slotmap` and `thunderdome`.

## What exists

- `nifeos/generational_table`: `Table<T, N>` and `EntryName<T>` with its `to_bits`, `from_bits` and
  `NEVER_MINTED`, every name ratified, about 820 lines with tests and
  harnesses, no dependencies and `#![forbid(unsafe_code)]`. Its mechanism is in [Generational names](../../notes/generational-names.md).
- Five Kani harnesses, each with a falsification patch under that repository's
  `falsifications/`. They prove five things. A removed name never resolves
  again. A dead name stays dead whatever operations follow. A name the table never minted resolves
  to nothing. Live names are distinct and resolve to their own entry. A bounded walk yields every
  live entry.
- The release record, [generational_table and the existing crates](../../notes/generational-table-and-existing-crates.md),
  answers whether a published crate already covers an allocation-free, fixed-capacity generational
  table. None does: `slotmap` needs an allocator and lets two `u64` names reach one entry, `coca` is
  dormant and `unsafe`-heavy, and `triple_arena`'s stack backing has no `const` constructor, one
  generation for the whole arena and `unsafe` storage.

Reuse: none exists that fits. Searched crates.io and read twelve candidates from source (`slotmap`, `thunderdome`, `coca`, `triple_arena`, `heapless` and seven more); the release record gives each verdict.

## The soak

- The crate lives in `nifeos/generational_table`, on the shared template of milestone 887 (crate
  repositories share one CI template) with its callers pinned at `nifeos/crate-workflows`
  `965f58a`. `script/apply-settings` applied the settings there: both rulesets, the `crates-io`
  environment with calef as reviewer, the labels, the team and private vulnerability reporting. The manifest
  keeps `publish = false`, and the caller sets `allow-unpublished` for the soak. No tag exists.
- nife keeps no copy. `kernel` and `memory_regions` depend on `nifeos/generational_table` by `rev`
  (§305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)), at `759c8f7`,
  a commit on its `main`. The verify table, the Kani reach list, the Kani peaks table and the mutation
  triage no longer list it; the repository's own `proofs / kani` and `proofs / falsify` jobs
  prove the five harnesses.
- End condition 1 holds once the repository's `main` is green at the pinned commit. Condition 2
  waits on the first weekly run, 3 on a release dry run, 4 on a recorded stranger run
  (`docs/stranger-test.md` there reads `Last run: none`), and 5 on four weeks from the tag nife
  pins.
- A bump of the pin is a separate pull request citing the landed commit as
  `nifeos/generational_table@<sha>`.

## What remains

1. The tag `v0.1.0`, which the release workflow makes when calef dispatches it, and the pin moved
   to that tag's commit.
2. The soak, which ends on evidence and not a duration, as in milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own).
3. The publication of 0.1.0 by trusted publishing, which waits for the architect's approval.
4. nife consuming the release by semantic version, with `Cargo.lock` recording its checksum, the
   kernel's seven tables and `crates/memory_regions` among the consumers.

The crate takes the third slot, after `file_allocation_table` (milestone
813) and `extensible_host_controller_interface` (milestone 820).

## BUGS

- The `nife-smelter` app is not installed on `nifeos/generational_table`, and the repository holds
  none of the organization secrets `NIFE_SMELTER_APP_ID`, `NIFE_SMELTER_PRIVATE_KEY` and
  `SETTINGS_TOKEN`. `script/apply-settings` refused the app (the API wants an organization owner).
  The release workflow's tag job and the weekly settings-drift job need them, so both wait on
  calef.

- Insert scans first-fit in O(N). The README states it.
- `[Option<T>; N]` spends a tag per slot on any `T` without a niche. The README states it.
- A slot retires after 2^32 - 1 occupants, which shrinks the table's capacity by one. The README
  states it.
- Measured demand for fixed-capacity tables is close to nothing, so the crate may find few users.
  The README says so.

## Follow-on

- **Outstanding.** The tag `v0.1.0`, the pin moved to its commit, the soak's end conditions 2 to 5
  and the publication. Checked by `gh api repos/nifeos/generational_table/tags`, which lists no
  tag, and `docs/stranger-test.md` there, which reads `Last run: none`.
- **Recorded.** An organization owner installs the `nife-smelter` app and grants the three secrets;
  see *BUGS* above.

## Index row

nife's generational table of named slots, with its five Kani proofs and their falsification patches, moves to its own `nifeos` repository. It publishes on crates.io as `generational_table` 0.1, the fixed-capacity, allocation-free member of the slotmap family. nife then consumes the release.
