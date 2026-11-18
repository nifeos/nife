---
status: PARTIAL
raised: 2026-11-09
milestone_dependencies: 1033
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1048. `generational_table`: what an outside maintainer would fix before 0.1.0

Raised by the lane `lane/generational-table-outside-review`, a read-only review of
`nifeos/generational_table` as a stranger to this tree would meet it. It changes nothing; it lists
what the next lanes change.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/generational-table.md) and
[the release record](../../notes/generational-table-and-existing-crates.md), and raises none of the decisions those settle. Those are the crate name and the 19 ratified names. They are the typed `EntryName<T>` with `to_bits`, `from_bits` and `NEVER_MINTED`, and the saturating generation that retires a slot. They are the first-fit insert and the `[Option<T>; N]` storage. They are the dual license, the low expected demand the README states, and the ruling that the crate publishes.
Milestone 1033 (`generational_table`: the
fixed-capacity, proved slotmap, released on its own) owns the tag, the soak and the publication.
This block owns the findings that should land inside that path.

Reuse: the template `nifeos/crate-workflows` and its scaffold, cargo-mutants, Kani, cargo-fuzz, and
the crate's own harness helpers (`any_reachable_table`, `any_operation`). No new tool.

## Goal

A stranger who finds `generational_table` on crates.io can tell in one screen what it is for. The same stranger can build and prove it from the README alone, reach for the methods a slotmap user expects, and trust that every safety claim holds on every target the crate compiles for.
The repository's gates are green
for the crate's own reasons and red only for the crate's own reasons.

## What was checked, and how

Verified firsthand, on a fresh clone at the head of `main`:

- `cargo build`, `cargo test` (23 unit tests, 5 documentation tests, 1 compile-fail test),
  `cargo clippy --all-targets` and `cargo doc` with warnings denied, all clean on stable 1.99.1 and
  on 1.88, the manifest's `rust-version`.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- Kani 0.68.0 verifies all six harnesses, and each of the six falsification patches applies and
  fails exactly its own harness at the assertion its header names.
- cargo-mutants over the whole crate: 83 mutants, 49 caught, 34 unviable, none missed, none timed
  out. The repository's own weekly shard reports the same figures.
- Both cargo-fuzz targets build on nightly and ran 25 seconds each with no finding (180,000 and
  2,000,000 executions).
- `typos` and `cargo deny check` pass. `cargo package --list` ships `src/`, the manifest, the
  lockfile, the README, the changelog and both license files.
- Sizes: `Table<u64, 256>` and `Table<*mut u8, 256>` are 5,136 bytes; `Table<u32, 256>` and
  `Table<&u8, 256>` are 3,088 bytes; `Option<EntryName<T>>` is 16 bytes.
- The repository's workflow runs, rulesets and the template's history, through the GitHub API.
- Download figures for the prior-art crates, from the crates.io API.

Inferred from reading, not run: the behavior on a target whose `usize` is narrower than 32 bits
(item 2), and whether the crate builds at Rust 1.85 (item 15).

Not run: `cargo semver-checks` (no previous release to compare against), Miri (the weekly job runs it and passed), a long fuzz campaign, a release dry run, `script/apply-settings` and the template's name-check helpers. The review package ran the helpers over the same names, and no public name has changed since.

## Prior art

How the crate sits against what a crates.io user already knows. Recent downloads are the last 90
days at the time of the review.

| crate | recent / all-time downloads | storage | handle | generation | what a user coming from it will miss here |
|---|---|---|---|---|---|
| `slab` 0.4 | 289M / 1.11G | `Vec` | `usize`, no generation | none | `insert(value)`, `capacity`, `clear`, `iter_mut`, `retain`, `Index`; and the README does not name it |
| `slotmap` 1.1 | 37.3M / 130M | `Vec`, growable | `KeyData`, `as_ffi`/`from_ffi`, two words can reach one entry | per slot, odd while occupied, wraps after 2^31 reuses | `insert`, `contains_key`, `iter_mut`, `capacity`, `clear`, `retain`, `drain`, `Index`, `Ord` on keys, secondary maps |
| `generational-arena` 0.2 | 1.22M / 10.7M | `Vec`, archived upstream | `(usize, u64)`, 16 bytes | one counter for the arena | `insert`, `contains`, `iter_mut`, `capacity`, `clear`, `retain`, `drain` |
| `thunderdome` 0.6 | 227k / 1.18M | `Vec` | `Index`, `to_bits`/`from_bits`, same packing as this crate | per slot, wraps past zero after 2^32 - 1 | `insert`, `contains`, `iter_mut`, `capacity`, `clear`, `retain`, `drain`, `Index::slot()`, `get_by_slot` |
| `triple_arena` 0.15 | 232k / 361k | inline on `StackBacking`, `unsafe` | a caller type from a macro, index and optional generation | one counter for the arena | growth, many arena kinds |
| `coca` 0.3 | 17 / 4.4k | inline, `MaybeUninit`, `unsafe`; dormant | `u64`, generation high, index low | per slot, wraps after 2^31 | constant-time insert, a packed dense pool |

Where this crate differs: capacity fixed at compile time, no allocation, a `const` constructor that
sits in a `static`, no `unsafe`, and a generation that retires a slot instead of wrapping. A typed name that any `u64` can spell, which the table checks on every use. A walk bounded by the highest live slot and resumable from a slot cursor. Six machine-checked proofs, each shipped with the patch that makes it fail. What it costs: an O(N) first-fit insert, one `Option` tag per slot on a `T`
without a niche, a slot lost after 2^32 - 1 occupants, and the smaller method surface in the last
column. The release record's Kani run against `slotmap` is the concrete example. A map holding one
entry resolves a `u64` it never minted, because `from_ffi` forces the version odd. This crate's
`a_name_the_table_never_minted_resolves_to_nothing` refuses that.

## The items

The fifteen items, graded P0 to P2 with evidence, change, door and effort, are in
[the appendix](1048-generational-table-outside-review/the-items.md). Item numbers in this block
refer to it.

## What needs the architect

No one-way door remains open. The architect ratified the five new public names of item 6 and
`Ord` on `EntryName<T>` (item 11). The ruling already covers the publication, its version, the
license and the crate name. This block does not reopen them.

## Done when

- `Cargo.toml` carries no comment that points outside the repository, and the release that
  publishes carries no `publish = false`.
- `slot_of` resolves nothing for a slot the target cannot index, with a test that spells such a
  name, and the documentation says so.
- The README has a section a stranger can follow to build, test, prove and falsify, the Kani install
  line copies as one line, and a new stranger run is on record with no open finding against the
  README.
- The callers pin a template release that carries the mutation stack, `mutation.yml` is a caller
  with this repository's cron, `check / mutation-diff` is a required check, and `mutation /
  mutation` has one green full run on `main`.
- Six harnesses become seven, each new one arriving with a falsification patch that `proofs /
  falsify` replays red, and the walk harness counts `values_mut`.
- `insert`, `contains_key`, `iter_mut`, `capacity` and `clear` exist with tests, and the README's
  method list names them.
- A benchmark in the repository reproduces every performance figure the README states, or the
  README states none.
- `cargo test`, `cargo kani`, `cargo mutants` (none missed) and both fuzz targets are green at the
  commit the release tags.

## BUGS

- Item 5 was the template's to fix, and `nifeos/crate-workflows#50` fixed it; the release gate
  reads the mutation verdict, which is green on `main`.
- Item 2 came from reading, not from a build for a 16-bit target. The fix is cheap enough to
  land without one. A `#[cfg(target_pointer_width = "16")]` test would need a toolchain this
  repository does not carry.
- The review did not run the template's name-check helpers again; the review package's run covers
  the same names, and the five new names of item 6 will meet them in their own pull request.
- The fuzz runs here took 25 seconds per target on one machine. The weekly job gives five minutes.
  Neither is a long campaign. The repository commits no seed corpus, so each run starts cold.

## Where this stands

Finished: items 2 and 5 to 15 apart from item 13's changelog line, item 3's pre-release half, and
item 4 apart from its pin (`nifeos/generational_table` #9 to #16 and #18, with
`nifeos/crate-workflows#50`). The ten harnesses carry ten falsification patches, `check /
mutation-diff` is a required check on `main`, and full mutation and fuzz runs are green on a `main`
whose `src/` and manifest match the `v0.1.0-pre.1` tag.

| item | what remains | blocker |
|---|---|---|
| 1 (P0) | delete `publish = false` in the release that publishes | publishing hold |
| 3 (P1) | the `cargo add` line and the crates.io and docs.rs badges | publishing hold |
| 4 (P1) | the callers pin `nifeos/crate-workflows` at `4d9eea2`, which is no template release; bump to the next one | release step |
| 13 (P2) | collapse the `0.1.0-pre.1` changelog section to one line at 0.1.0 | release step |
| done when, last bullet | `cargo test`, `cargo kani`, `cargo mutants` and both fuzz targets green at the commit the 0.1.0 release tags | release step |

No item needs a decision for calef.

## Follow-on

Read against `nifeos/generational_table` `main` at `1a8f8dc`, the merge of #18.

- **Done.** Items 2, 5 to 12, 14 and 15 landed in `nifeos/generational_table` #9 to #16. The slot
  check on narrow targets is #9 and #14, and the five methods and `Ord` are #10. The docs.rs front
  page, `deny.toml` and `rust-version` 1.85 are #11. The benchmark, the cursor scan and the layout
  record are #12. The mutation caller, with the template fix of `nifeos/crate-workflows#50`, is #15
  and #16.
  Ten harnesses carry ten falsification patches, and proofs run 43001273410 passed at `4fb6bb7`.
- **Done.** Item 3's pre-release part and item 13's text fixes landed in
  `nifeos/generational_table` #11: the README's build-and-prove section, the one-line Kani install,
  a stranger run that found no gap, the `slab` comparison, the CONTRIBUTING line and the CODEOWNERS
  caveat.
- **Done.** Item 4 apart from its pin: the `mutation-diff 30` budget line is #18, `check /
  mutation-diff` is among the `main` ruleset's required checks, and mutation run 43928163330 is
  green on `main`.
- **Done.** The fuzz half of the last done-when bullet for the pre-release: fuzz run 44126438080 is
  green at `551fb87`, whose `src/` and manifest are those of the tagged `4fb6bb7`.
- **Outstanding.** Item 1's second half and item 3's release half: `publish = false` stays and the
  README has no `cargo add` line or badges until the release that publishes, which the publishing
  hold keeps back. Read from `Cargo.toml` and the README at `1a8f8dc`.
- **Outstanding.** Item 4's pin: every caller pins `nifeos/crate-workflows` at `4d9eea2`, 178
  commits past `v7`, the newest template release. Read from the workflow files at `1a8f8dc` and
  the template's tags.
- **Outstanding.** Item 13's changelog collapse and the last done-when bullet both belong to the
  0.1.0 release: the `0.1.0-pre.1` section of `CHANGELOG.md` still holds six entries. Read at
  `1a8f8dc`.

## Index row

An outside review of `generational_table` before its first release: one manifest fix that blocks publishing, seven items to land before the tag, and seven for later. The seven cover a slot truncation on narrow targets, the README's recorded gaps, the template's mutation stack and its single-shard verdict bug, five missing methods, an induction-step proof for the walk bound, and a benchmark behind the README's figure.
