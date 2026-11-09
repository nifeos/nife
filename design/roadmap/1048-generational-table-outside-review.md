---
status: NOT-STARTED
promoted_from: generational-table-outside-review
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

The review took the accepted baseline from
[the review package](../../notes/crate-reviews/generational-table.md) and
[the release record](../../notes/generational-table-and-existing-crates.md), and raises none of the
decisions those settle: the crate name, the 19 ratified names, the typed `EntryName<T>` with
`to_bits`, `from_bits` and `NEVER_MINTED`, the saturating generation that retires a slot, the
first-fit insert, the `[Option<T>; N]` storage, the dual license, the low expected demand stated in
the README, and the ruling that the crate publishes. Milestone 1033 (`generational_table`: the
fixed-capacity, proved slotmap, released on its own) owns the tag, the soak and the publication; this
block owns the findings that should land inside that path.

Reuse: the template `nifeos/crate-workflows` and its scaffold, cargo-mutants, Kani, cargo-fuzz, and
the crate's own harness helpers (`any_reachable_table`, `any_operation`). No new tool.

## Goal

A stranger who finds `generational_table` on crates.io can tell in one screen what it is for, build
and prove it from the README alone, reach for the methods a slotmap user expects, and trust that
every safety claim holds on every target the crate compiles for. The repository's gates are green
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

Not run: `cargo semver-checks` (no previous release to compare against), Miri (the weekly job runs
it and passed), a long fuzz campaign, a release dry run, `script/apply-settings`, and the
template's name-check helpers (the review package ran them over the same names, and no public name
has changed since).

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
sits in a `static`, no `unsafe`, a generation that retires a slot instead of wrapping, a typed name
that any `u64` can spell and that the table checks on every use, a walk bounded by the highest live
slot and resumable from a slot cursor, and six machine-checked proofs each shipped with the patch
that makes it fail. What it costs: an O(N) first-fit insert, one `Option` tag per slot on a `T`
without a niche, a slot lost after 2^32 - 1 occupants, and the smaller method surface in the last
column. The release record's Kani run against `slotmap` is the concrete example: a map holding one
entry resolves a `u64` it never minted, because `from_ffi` forces the version odd; this crate's
`a_name_the_table_never_minted_resolves_to_nothing` refuses that.

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a public name, a published behavior, a
layout, a license); everything else is two-way and the maintainer decides it.

### P0: blocks any publish

1. **The manifest's publish line and the comment beside it.**
   Finding: `publish = false` is deliberate for the soak, and the comment on that line sends the
   reader to a file that is not in this repository.
   Evidence: `Cargo.toml` line 22.
   Change: now, replace the comment with one sentence a stranger can act on ("set to `true` only in
   the release that publishes"). At the release, delete the line. The release itself waits on the
   approval milestone 1033 records; nothing here reopens that.
   Door: the comment is two-way; publishing is one-way and already ruled. Effort: XS.

### P1: before the first release

2. **The slot index truncates on a target whose `usize` is narrower than 32 bits.**
   Finding: `unpack` reads the slot as `(bits & 0xffff_ffff) as usize`. On a 16-bit target the cast
   drops the high half, so a name with slot `0x1_0000` and the right generation resolves to slot 0.
   The `const` guard in `new` only bounds `N`, which a 16-bit `usize` satisfies trivially. The
   proofs run on a 64-bit host and cannot see it. No target the crate names is affected, and the
   crate's three bare-metal targets are all 64-bit, so this is a hole in the claim rather than a
   defect a current user can reach.
   Evidence: `src/lib.rs` lines 203 to 206 and 247; `src/lib.rs` line 186.
   Change: make the slot conversion fallible, `usize::try_from(bits as u32).ok()?` in `slot_of`,
   so a slot the target cannot index resolves to nothing on every target; add one sentence to the
   `EntryName` documentation. The alternative, a `const` assertion `usize::BITS >= 32` in `new`,
   refuses those targets outright; the fallible conversion keeps the claim everywhere and costs one
   compare on a path that already compares.
   Door: two-way (no behavior changes on any supported target). Effort: XS.

3. **The README's recorded gaps are still open.**
   Finding: the stranger run recorded five findings against the README and each still stands: no
   build or test command; the Kani install command is split across two lines inside one code span,
   so it copies badly; the falsification patches are described but not how to apply one, run one
   harness, or undo it; a passing `cargo kani` still prints warnings that a newcomer reads as
   failure; and the run never exercised the stated minimum Rust version (the weekly `msrv` job
   does, which the README can say).
   Evidence: `docs/stranger-test.md` lines 28 to 37; `README.md` lines 70 to 71 and 92 to 96.
   Change: a "Building and proving" section with `cargo build`, `cargo test`, the install line on
   one line, `cargo kani`, and the three commands around a patch (`git apply`, `cargo kani --harness
   <name>`, `git checkout -- .`); one sentence on what a green run prints. At the release, add a
   `cargo add generational_table` line and the crates.io and docs.rs badges to the row, which the
   badge job will expect. Then record a new stranger run.
   Door: two-way. Effort: S.

4. **The template pin is current, and the next template release brings the mutation stack.**
   Finding: every caller pins `nifeos/crate-workflows` at `e0c1fcad`, which is ahead of the
   template's newest release, so the `lag` job passes. The template's head is 31 commits further
   on, all unreleased, and they add what this repository does not yet have: a `mutation.yml` caller
   (the full run moves out of `weekly.yml`), `check / mutation-diff` on every pull request as a
   required check, a release gate that refuses a release without a green full mutation run at the
   released commit, and a `mutation-diff 30` budget line. The repository's ruleset today requires
   nine checks and not that one.
   Evidence: `.github/workflows/ci.yml` line 17 and the other five callers; `.github/ci-job-budgets`;
   the template's `scaffold/caller-workflows/mutation.yml` and `.github/ci-job-budgets` at head; the
   `main` ruleset's required checks.
   Change: when the template releases, take Dependabot's pin bump and in the same pull request add
   the scaffold's `mutation.yml` caller with the repository's own cron line
   (`helpers/mutation_plan.py schedule generational_table` prints it), copy the scaffold's budgets
   file, then run `script/apply-settings` so the tenth check becomes required. Dispatch
   `mutation.yml` once on `main` before the release so the gate has a green full run to read.
   Door: two-way. Effort: S.

5. **The weekly mutation verdict is red for a template reason, and it will block the release gate.**
   Finding: the first weekly run's `mutation` verdict failed with "0 of 1 shards left an
   outcomes.json" while the one shard succeeded and uploaded `outcomes.json` at the root of its
   artifact. The verdict downloads with `pattern: mutants-shard-*` and counts `outcomes.json` at
   depth two. When the pattern matches one artifact, `download-artifact` extracts it directly into
   the target directory (the job log says "download of artifact to: .../mutants-shards"); when it
   matches two, each lands in its own subdirectory (a two-shard repository's log says
   ".../mutants-shards/mutants-shard-0" and its verdict passed). So every crate with at most 200
   mutants fails the verdict, and every single-shard repository in the organization went red the
   same morning. The template's head carries the same `find` in `mutation.yml`. The crate's own
   mutation result is clean: 83 mutants, 49 caught, 34 unviable, none missed.
   Evidence: weekly run `42657660671`, verdict job `129300873401` and shard job `129300499028`;
   the template's `weekly.yml` at the pin, line 332, and `mutation.yml` at head, line 269; the
   shard artifact `13693695382`, whose listing has `outcomes.json` at the root.
   Change: in the template, make the verdict count `outcomes.json` at depth one or two, or download
   each shard into an explicit `path` of its own. Nothing changes in this repository; re-run the
   workflow after the template fix. Owner: the template, through its own queue; this block names
   the dependency on the blocked side.
   Door: two-way. Effort: S in the template.

6. **The methods a slotmap user reaches for first are missing.**
   Finding: the table has `insert_with`, `get`, `get_mut`, `remove`, `slot_of`, `len`, `is_empty`,
   `values`, `values_mut`, `live_slots`, `iter` and `iter_from`. Every crate in the prior-art table
   also has `insert(value)`, a membership test, `iter_mut`, `capacity` and `clear`. A user writing
   `insert_with(|_| value)` for the common case, or `get(name).is_some()` for membership, or
   `values_mut()` when the sweep also needs the name, will count these as gaps on the first day.
   Each is a few lines over what exists and none changes a current method.
   Evidence: `src/lib.rs` lines 182 to 373, the whole `impl` block.
   Change: add `insert(value) -> Option<EntryName<T>>`, `contains_key(name) -> bool`,
   `iter_mut() -> (EntryName<T>, &mut T)`, `capacity() -> usize` (returns `N`, for generic code)
   and `clear()` (takes every live entry and bumps its slot's generation, so every outstanding name
   dies, and brings `top` to zero), each with a test and a README line. The spellings are the
   standard library's where it has one and the family's where it does not; they are provisional
   until ratified, and they ship in 0.1.0, so they belong in the release's name review.
   Door: two-way until the tag; one-way after. Effort: S.

7. **The walk bound and the live count have no induction step.**
   Finding: `a_bounded_walk_yields_every_live_entry` proves the bound over one fixed sequence
   (three inserts, any subset removed) and `a_removed_name_stays_dead_whatever_comes_after` carries
   the generation invariant forward from any reachable table. Nothing carries the `top` invariant
   (every slot at or above `top` is `None`) or `live == count of Some` forward from any reachable
   table through any operation, which is what makes the bound trustworthy after a long run. The
   walk harness also leaves out `values_mut`, the one walk with its own slice.
   Evidence: `src/verification.rs` lines 37 to 61 (the reachable table and the operation) and 127
   to 156 (the walk harness); `src/lib.rs` lines 306 to 309.
   Change: one harness, `the_walk_bound_and_the_live_count_survive_any_operation`: from
   `any_reachable_table`, after `any_operation`, assert `slots[top..]` holds no `Some`, `live`
   equals the count of `Some`, and `values().count() == len()`; add `values_mut().count()` to the
   existing walk harness. Each new harness arrives with its falsification patch, as the proofs
   workflow demands.
   Door: two-way. Effort: S.

8. **A performance figure with nothing to reproduce it.**
   Finding: the README and the crate documentation state that an insert-and-remove cycle takes
   about 7 nanoseconds on a 2.1 gigahertz server core, which is how they argue that a slot can
   retire in about half a minute. The repository has no benchmark, so a reader cannot check the
   figure or re-measure it on another machine. Neither the first-fit scan nor a lookup is measured
   anywhere.
   Evidence: `README.md` lines 104 to 105; `src/lib.rs` lines 14 to 16.
   Change: a `benches/` target with `harness = false` and no new dependency (`std::time::Instant`
   and `std::hint::black_box`), measuring insert-remove cycles at N of 16 and 4,096, a lookup, and
   the scan into a table that is nearly full; the README then states the figure with its method and
   the N it was measured at, or drops the figure and keeps the arithmetic (2^32 cycles at the
   measured rate).
   Door: two-way. Effort: S.

### P2: later

9. **The first-fit scan is O(N) on every insert.**
   Finding: `insert_with` scans from slot 0 each time. A table of 4,096 that stays nearly full
   pays a few thousand compares per insert. Every other crate in the family keeps a free list; the
   README states the cost.
   Evidence: `src/lib.rs` lines 224 to 229.
   Change: measure first (item 8). If the scan shows, keep a lowest-free-slot cursor: insert scans
   from the cursor and moves it up past the slot it fills; remove lowers it to the freed slot when
   that is lower. First-fit order is unchanged, so every name the table would mint stays the same,
   and the worst case stays O(N) while the common case stops paying it. Four bytes of state.
   Door: two-way (no public behavior changes). Effort: M.

10. **The storage layout is measured but not documented, and not compared.**
    Finding: generations and entries live in two arrays, so a lookup touches two cache lines and
    the insert scan reads two streams. `Table<u64, 256>` is 5,136 bytes and `Table<u32, 256>` or a
    table of references 3,088, which is the `Option` tag's cost on a `T` without a niche. The README
    says a tag is spent; it does not give the formula or a figure.
    Evidence: `src/lib.rs` lines 166 to 180; the sizes above.
    Change: state the size in `BUGS` as `N * (size_of::<Option<T>>() + 4)` plus two words, with one
    measured example, and add a `size_of` test that pins it. Then, with item 8's benchmark in hand,
    try one array of `(u32, Option<T>)` and keep whichever measures better. Storage is private, so
    the layout is not a commitment.
    Door: two-way. Effort: S for the documentation, M for the comparison.

11. **`EntryName` as a key in other collections.**
    Finding: `EntryName<T>` has `PartialEq`, `Eq`, `Hash`, `Copy` and `Debug` for every `T` and no
    `Ord`, so it cannot key a `BTreeMap` or sort a list of names; `slotmap` and `thunderdome` keys
    can. Its `Debug` writes one string by hand rather than through `debug_struct`, so the alternate
    form (`{:#?}`) does not indent it inside a pretty-printed table.
    Evidence: `src/lib.rs` lines 100 to 132.
    Change: `PartialOrd` and `Ord` by the packed bits (the only order the type can have), and
    `debug_struct("EntryName")` with the two fields. Leave out raw `slot()` and `generation()`
    accessors: `slot_of` already gives the validated slot, and an unvalidated accessor invites the
    stale-index bug the type exists to prevent. Adding a trait implementation is compatible;
    removing one is not, so `Ord` is a small one-way door.
    Door: `Ord` one-way once published; the `Debug` form two-way. Effort: XS.

12. **The crate's front page on docs.rs shows no code.**
    Finding: the crate-level documentation is prose and limits; the example lives in the README,
    which docs.rs does not render, and in `Table`'s documentation one click away. Two facts a user
    needs are also missing from the documentation: `N` must be between 1 and `u32::MAX` (the
    `const` assertion refuses `N == 0` at compile time), and a large table built as a local sits on
    the stack, so it belongs in a `static` or must be built in place.
    Evidence: `src/lib.rs` lines 1 to 25 and 186; `README.md` line 50.
    Change: a ten-line example in the crate documentation, one sentence on `N`, one on the stack.
    Door: two-way. Effort: XS.

13. **Four small text fixes for a stranger.**
    Finding: the README's comparison never names `slab`, which is where most people meet the shape
    (289M downloads in 90 days) and which has no generation at all, so a stale index reaches the
    next occupant. The changelog's `Unreleased` lists the first version and then one addition to
    it, which at 0.1.0 is one line. CONTRIBUTING says the crate stays "free of vocabulary from any
    other tree", which means nothing to an outside reader; say that the `standalone` job refuses
    references to any other repository. CODEOWNERS says the maintainers' team may not exist; it
    does.
    Evidence: `README.md` lines 113 to 156; `CHANGELOG.md` lines 6 to 9; `CONTRIBUTING.md` line 17;
    `CODEOWNERS` lines 1 to 3.
    Change: one sentence on `slab` in the comparison; collapse the changelog entry at release;
    reword the CONTRIBUTING line; drop the CODEOWNERS caveat.
    Door: two-way. Effort: XS.

14. **The `Zlib` allowance in `deny.toml` matches nothing.**
    Finding: `cargo deny check` warns that the `Zlib` allowance is unmatched; the crate has no
    dependency, so every allowance is unmatched, and this is the one the tool reports. It is the
    scaffold's list, so it is harmless and a stranger running the check sees a warning.
    Evidence: `deny.toml` line 22.
    Change: none in this repository unless the template trims the list for a dependency-free crate;
    note it here so nobody chases it.
    Door: two-way. Effort: XS.

15. **`rust-version` may be higher than the code needs.**
    Finding: the manifest states 1.88. Edition 2024 needs 1.85, and nothing read in `src/lib.rs`
    visibly needs a later release (inline `const` blocks and `const { None }` array repeats are
    older). A lower stated minimum widens who can take the crate, and lowering it later is
    compatible while raising it is a change users feel. Not verified by a build.
    Evidence: `Cargo.toml` lines 4 to 5.
    Change: build and test at 1.85; if green, state 1.85 and let the weekly `msrv` job hold it.
    Door: two-way. Effort: XS.

## What needs the architect

Only the one-way doors. Everything else above is the maintainer's to decide and a lane's to land.

- The five new public names of item 6 (`insert`, `contains_key`, `iter_mut`, `capacity`, `clear`)
  ship in 0.1.0 and become part of what users act on. They are provisional, chosen by the standard
  library's spelling where it has one, and belong in the release's name review with the 19 already
  ratified.
- `Ord` on `EntryName<T>` (item 11), a trait implementation a later release cannot remove.

The publication itself, its version, the license and the crate name are ruled and not reopened
here.

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

- Item 5 is the template's to fix. Until it lands, this repository's weekly `mutation` verdict stays
  red for a reason no change here removes, and after the pin bump the release gate reads that
  verdict.
- Item 2 was verified by reading, not by a build for a 16-bit target; the fix is cheap enough to
  land without one, and a `#[cfg(target_pointer_width = "16")]` test would need a toolchain this
  repository does not carry.
- The review did not run the template's name-check helpers again; the review package's run covers
  the same names, and the five new names of item 6 will meet them in their own pull request.
- The fuzz runs here were 25 seconds per target on one machine; the weekly job gives five minutes.
  Neither is a long campaign, and no seed corpus is committed, so each run starts cold.

## Index row

An outside review of `generational_table` before its first release: one manifest fix that blocks
publishing, seven items to land before the tag (a slot truncation on narrow targets, the README's
recorded gaps, the template's mutation stack and its single-shard verdict bug, five missing methods,
an induction-step proof for the walk bound, a benchmark behind the README's figure), and seven for
later.
