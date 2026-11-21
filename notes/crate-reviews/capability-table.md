# Review package: `capability_table`

*Name: provisional, minted by lane milestone/1081-capability-table-review-package. The
`capability_table` package of [crate-reviews.md](../crate-reviews.md), in its own file because the
packages together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository. The findings it
rests on are in [milestone 1081 (`capability_table`: what an outside maintainer would fix before it
leaves)](../../design/roadmap/1081-capability-outside-review.md), whose Open decisions section asks
for exactly this package. The architect cleared it on 2026-11-21 (UTC), option A, and the crate
below is the one that moves.

## In one paragraph

A fixed-size table of capabilities, the kernel's file descriptor table that can point at anything.
A capability is an object and a set of five rights (`READ`, `WRITE`, `GRANT`, `ENUMERATE`, `BIND`).
The table has at most 64 slots, holds a free-slot word with a set bit for each empty slot, and a
program sees only a slot number. Passing a capability on goes through one function,
`Capability::delegate`, which needs `GRANT` and can only narrow. A process-wide peak counter sits
behind the `peak` cargo feature. It is `no_std`, has no dependency and no `unsafe`, and 12 Kani
harnesses prove the rights algebra, the slot bookkeeping and the delegation rule, each with a
falsification patch. The kernel is its only consumer. The two supervision decisions
(`reap_decision`, `survey_includes`) stay in the kernel, in `kernel/src/sched/region_reaping.rs`,
with their four harnesses.

## Identity

| item | value |
|---|---|
| crate | `capability_table`, ratified 2026-08-01 as `capability` and renamed on the architect's ruling of 2026-11-16 (milestone 1081 D1) |
| identity values | none: no `INTERFACE_ID`, `VERSION` or magic word. The crate is a library the kernel links, not a wire contract; `abi::rights` pins the five right bits and `kernel/src/capability_object.rs` asserts them equal |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.85 |
| license | `MIT OR Apache-2.0`, with both texts. The crate adapts nothing from outside |
| `publish` | `false`; the crate is not on `crates/publish-allowlist.txt` |
| size | `src/lib.rs` 1,025 lines and `src/tests.rs` 589 lines; 19 tracked files, 16 of them falsification patches |
| mutation baseline | `.cargo/mutants-baseline.txt` has the row `capability_table 98 3 3 26` (caught, missed, timeout, unviable) from its census; the crate's own `.github/mutants-exempts` lists each survivor and timeout with its reason |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `kernel` only (`kernel/Cargo.toml`, with `version = "0.1.0"` and the feature `peak`) |
| source files that call it | `kernel/src/capability_object.rs` (which keeps the alias `CapabilityTable<Object, CAPABILITY_TABLE_SLOTS>` and calls `Capability::delegate` from `Delegation::derive`), and `mint_child` through the retype and memory-region system calls |
| other files that name the crate | 24, listed by the dry run: workflows, helper tables and selftests, `script/falsifications`, `script/lint`, `script/verify`, `packages/kernel.package.toml`, `SECURITY.md`, two bench logs, `helpers/trust_boundary_crates.tsv`, `helpers/template_report_baseline.txt` and six `system_tests` files and falsification patches. The move lane reads each |
| fuzz, std farm | none: no fuzz target names the crate, and the standard library copies no source file |

No test reads a `../../` file and no `include_str!` leaves the crate.

## What was run

All results are from this lane, on `main` plus this pull request, on a Linux host.

| check | result |
|---|---|
| `script/crate-standalone capability_table` | 8 failures, all known and all for the move: `names` (37 names not ratified, 36 of them with no `Name:` block and the rest provisional), manifest inherits `license`, manifest inherits `lints`, no `rust-version`, no README, no `LICENSE-MIT`, no `LICENSE-APACHE`, one citation of a path into the tree (`src/tests.rs` line 291 names `script/verify`). The build on stable for the host and `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` passes; Kani proves 16 of 16 harnesses; all 16 falsification patches turn their harness red. Run time 1 minute 32 seconds |
| `script/crate-move --dry-run --no-build`, template `crate-workflows` at its release `v8` | 7 findings for the move lane, none new: the README is missing, the manifest inherits two keys and has no `rust-version`, `standalone` repeats the file findings, `prose caps` fires on the staged scaffold README, and the same one citation. Every one of the 16 patches applies with no offset and no fuzz once the move strips the `Name:` blocks. Note: no mutation baseline row for `capability_table` |
| `cargo test -p capability_table` | 26 unit tests and 2 doctests pass |
| `cargo clippy -p capability_table --all-targets -- -D warnings`, `cargo doc` with warnings denied | no warning |
| the template's naming checks at its head, over the staged `src/lib.rs` | `spelled_out`, `unit_symbols`, `question_shape`, `british_spelling` and `contract_names`: 164 names judged, 0 refused. `abbreviations`, report only: one component, `liveness` in a harness name |
| `script/names capability_table` | the crate name ratified, `MAXIMUM_SLOTS` ratified, the module `tests` ratified; `Capability`, `delegate`, `capacity` and `iter` read provisional; the other items have no block |
| `script/crate-review-packet capability_table` | the facts above and the API table below; 16 harnesses, 16 patches, one dependent, no `unsafe` site, no foreign type in a public signature |

Not run, and so not claimed:

- The stranger test, because the crate has no README to follow.
- A mutation census. The recorded counts and the three survivors' reasons in
  `notes/project-metrics/mutation-triage.csv` are read, not rerun.
- Miri; `cargo-semver-checks` (a first release has no baseline); the template's `check`, `proofs`
  and `weekly` workflows, which run in the repository; a release dry run; `script/crate-move`
  with a build (the dry run used `--no-build`).
- A 32-bit build. The host has no 32-bit target, so the slot truncation fix of milestone 1081's
  item 4 rests on its test and on reading.
- The template's `function_length`, `history_phrases` and `inflated_wording` at head were not run
  by hand; the dry run ran them at release `v8` and they pass.
- The kernel's QEMU suite, which the merge queue runs.
- `script/lint` at the pull request's tip: see the pull request's checks.

## Public API

| kind | items |
|---|---|
| rights | `Rights` with `NONE`, `READ`, `WRITE`, `GRANT`, `ENUMERATE`, `BIND`, `ALL`; `bits`, `from_bits`, `union`, `intersect`, `allows`, `is_subset_of` |
| capability | `Capability<O>` with public fields `object` and `rights`; `delegate`, `mint_child` |
| errors | `Error` (`NoSuchSlot`, `CannotWiden`, `GrantRequired`, `NoFreeSlot`, `SlotOccupied`), with `Display` and `core::error::Error` |
| table | `CapabilityTable<O, const N: usize>`, `MAXIMUM_SLOTS` (64); `new`, `used`, `peak`, `capacity`, `is_empty`, `get`, `insert`, `insert_at_or_above`, `insert_at`, `put`, `derive`, `delete`, `iter`, `delete_matching`; `Default` |
| process-wide | `highest_seen() -> (peak, ceiling)`, behind the `peak` feature and `target_has_atomic = "32"` |

No nife type appears in a public signature. What a stranger may find surprising:

- `CapabilityTable::derive` files a delegated capability into a second slot, and
  `Capability::delegate` holds the rule. The table method calls the capability method, so the
  rule has one body. The two `REVOKE` arms in `kernel/src/system_call.rs` test `GRANT` inline,
  under no harness, for the reason written beside them (finding P1-3 of milestone 1081).
- A table's slot number is a `u64`, and a slot past the table's length is `NoSuchSlot` on every
  target, not a truncated index.
- `highest_seen` is one process-wide value across every table of every size, which suits a kernel
  and surprises a library user, so it is behind the `peak` feature, off by default (finding P2-11).
- `Capability<O>` has no niche, so `Option<Capability<O>>` costs extra bytes (finding P2-16,
  optional).

## Rulings

- The crate's name is `capability_table` (the architect, 2026-11-16 UTC, milestone 1081 D1, option
  A). `capability` belongs on crates.io to an unrelated crate, and a rename after the move would be
  a second move. The review refuses a `nife_` prefix and adds no `#[doc(alias)]` for the old name.
- The five names `Capability::delegate`, `Error::GrantRequired`, `Error::SlotOccupied`,
  `CapabilityTable::capacity` and `CapabilityTable::iter` stand as built (the architect,
  2026-11-19 UTC, milestone 1081 D2, option A). The ruling refuses `derive` as a rename of `delegate`, and
  adds no `len` because in std it counts the elements held.
- The review is cleared (the architect, 2026-11-21 UTC, milestone 1081, option A): the crate moves
  with `publish = false`, no tag and no publish. `Reap`, `reap_decision` and `survey_includes`, their
  harnesses and their patches stay in nife, which answers open question 1.
- No table has more than 64 slots, so the exact `(64, 64)` pin in the global-peak test cannot
  exceed it; finding P2-14 is declined.
- The five right bits and their positions, the set-means-free word, the 64-slot ceiling and the
  publishing hold stand, and this package does not reopen them.

## Names

The names table follows `script/names capability_table`. This lane ratifies nothing in it.
The first three rows are the architect's rulings. Every row marked for the architect is the
architect's to ratify, as built.

| item | status | note |
|---|---|---|
| `capability_table` | ratified | crate; D1 of milestone 1081 |
| `MAXIMUM_SLOTS`, module `tests` | ratified | |
| `Capability::delegate`, `Error::GrantRequired`, `Error::SlotOccupied`, `CapabilityTable::capacity`, `CapabilityTable::iter` | ratified 2026-11-19 UTC (D2) | |
| `Capability` | for the architect to ratify | spelled out from `Cap` under the capability ruling of 2026-10-06 UTC; the block still reads provisional |
| `Rights`, `NONE`, `READ`, `WRITE`, `GRANT`, `ENUMERATE`, `BIND`, `ALL`, `bits`, `from_bits`, `union`, `intersect`, `allows`, `is_subset_of` | for the architect to ratify | no block; the five bit positions are the system call wire format, so the right names are the vocabulary `abi::rights` also uses; `ALL` is built from the five and is no longer a literal |
| `Capability::mint_child` | for the architect to ratify | the inheriting mint: same rights, new object. the narrowing, GRANT-checked path already uses `derive` and `delegate`, so `mint_child` is not a synonym |
| `Error::NoSuchSlot`, `CannotWiden`, `NoFreeSlot` | for the architect to ratify | no block |
| `CapabilityTable::new`, `used`, `peak`, `is_empty`, `get`, `insert`, `insert_at`, `delete` | for the architect to ratify | the standard-library words (D2 excludes `len`); `used` counts occupied slots and `peak` is the highest `used` ever seen |
| `CapabilityTable::insert_at_or_above`, `put` | for the architect to ratify | `insert_at_or_above` takes the lowest free slot at or above a floor; `insert_at` returns the slot and refuses an occupied one, `put` overwrites and returns `()`; the two names stay apart (the maintainer's provisional ruling on open question 2) |
| `CapabilityTable::derive` | for the architect to ratify | files `Capability::delegate`'s result in a second slot; the same word that D2 refused for the capability method now names the table method |
| `CapabilityTable::delete_matching` | for the architect to ratify | takes `FnMut` |
| `highest_seen` | for the architect to ratify | process-wide, behind the `peak` feature |
| feature `peak` | provisional | the maintainer's, for P2-11; joins the release name review |

Names that would follow a rename elsewhere: the kernel's alias `CapabilityTable<Object,
CAPABILITY_TABLE_SLOTS>` and its call sites read `capability_table::` already. No other crate
defines these names.

## What still blocks the move

| item | owner |
|---|---|
| Create nifeos/capability_table from `script/crate-move`, grant its secrets, push, apply the template settings and reach green runs | move lane |
| Convert the kernel's dependency to `git` and `rev` with `version = "0.1.0"` at a commit on the new repository's main, drop the crate from the workspace members, and read the other files the dry run lists | move lane |
| Run the stranger test and the template's workflows in the new repository | move lane |

## Proposed items

All are two-way until the first release, and the maintainer rules them provisionally.

1. An `Option<Capability<O>>` niche (P2-16). Optional; it changes the input domain of every rights
   harness, so a lane measures it before it takes it. Effort M.
2. A pedantic-clippy pass for `# Errors` sections and `#[must_use]`, which the milestone counted
   at eight and twelve findings. Effort XS.

## Open questions

1. Does the kernel's dependency on a crate it pins at a commit make the table's `MAXIMUM_SLOTS` of
   64 a number the kernel can outgrow? Growing past 64 is a wider word or a second one, and the
   const assertion in `CapabilityTable::new` catches a larger `N` at compile time.

`insert_at` and `put` stay two names (the maintainer's provisional ruling): one refuses an occupied
slot and returns the slot, the other overwrites and returns nothing, and the kernel needs both.
