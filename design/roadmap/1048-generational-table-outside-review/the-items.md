# Milestone 1048 appendix: the items

An appendix to [milestone 1048 (`generational_table`: what an outside maintainer would fix before 0.1.0)](../1048-generational-table-outside-review.md).

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a public name, a published behavior, a
layout, a license); everything else is two-way and the maintainer decides it.

### P0: blocks any publish

1. The manifest's publish line and the comment beside it.
   Finding: `publish = false` is deliberate for the soak, and the comment on that line sends the
   reader to a file that is not in this repository.
   Evidence: `Cargo.toml` line 22.
   Change: now, replace the comment with one sentence a stranger can act on ("set to `true` only in
   the release that publishes"). At the release, delete the line. The release itself waits on the
   approval milestone 1033 (`generational_table`: the fixed-capacity, proved slotmap, released on its own) records; nothing here reopens that.
   Door: the comment is two-way; publishing is one-way and already ruled. Effort: XS.

### P1: before the first release

2. The slot index truncates on a target whose `usize` is narrower than 32 bits.
   Finding: `unpack` reads the slot as `(bits & 0xffff_ffff) as usize`. On a 16-bit target the cast
   drops the high half, so a name with slot `0x1_0000` and the right generation resolves to slot 0.
   The `const` guard in `new` only bounds `N`, which a 16-bit `usize` satisfies trivially. The
   proofs run on a 64-bit host and cannot see it. No target the crate names is affected. The crate's three bare-metal targets are all 64-bit. So this is a hole in the claim, not a defect a current user can reach.
   Evidence: `src/lib.rs` lines 203 to 206 and 247; `src/lib.rs` line 186.
   Change: make the slot conversion fallible, `usize::try_from(bits as u32).ok()?` in `slot_of`,
   so a slot the target cannot index resolves to nothing on every target; add one sentence to the
   `EntryName` documentation. The alternative, a `const` assertion `usize::BITS >= 32` in `new`,
   refuses those targets outright; the fallible conversion keeps the claim everywhere and costs one
   compare on a path that already compares.
   Door: two-way (no behavior changes on any supported target). Effort: XS.

3. The README's recorded gaps are still open.
   Finding: the stranger run recorded five findings against the README and each still stands. The README gives no build or test command. The Kani install command breaks across two lines inside one code span, so it copies badly. The README describes the falsification patches but not how to apply one, run one harness, or undo it. A passing `cargo kani` still prints warnings that a newcomer reads as failure. The run never exercised the stated minimum Rust version, which the weekly `msrv` job does and the README can say.
   Evidence: `docs/stranger-test.md` lines 28 to 37; `README.md` lines 70 to 71 and 92 to 96.
   Change: a "Building and proving" section with `cargo build`, `cargo test`, the install line on
   one line, `cargo kani`, and the three commands around a patch (`git apply`, `cargo kani --harness
   <name>`, `git checkout -- .`); one sentence on what a green run prints. At the release, add a
   `cargo add generational_table` line and the crates.io and docs.rs badges to the row, which the
   badge job will expect. Then record a new stranger run.
   Door: two-way. Effort: S.

4. The template pin is current, and the next template release brings the mutation stack.
   Finding: every caller pins `nifeos/crate-workflows` at `e0c1fcad`, which is ahead of the
   template's newest release, so the `lag` job passes. The template's head is 31 commits further on, all unreleased. They add what this repository does not yet have. One is a `mutation.yml` caller
   (the full run moves out of `weekly.yml`). One is `check / mutation-diff` on every pull request as
   a required check. One is a release gate that refuses a release without a green full mutation run
   at the released commit. The last is a `mutation-diff 30` budget line. The repository's ruleset today requires
   nine checks and not that one.
   Evidence: `.github/workflows/ci.yml` line 17 and the other five callers; `.github/ci-job-budgets`;
   the template's `scaffold/caller-workflows/mutation.yml` and `.github/ci-job-budgets` at head; the
   `main` ruleset's required checks.
   Change: when the template releases, take Dependabot's pin bump and in the same pull request add the scaffold's `mutation.yml` caller with the repository's own cron line (`helpers/mutation_plan.py schedule generational_table` prints it). Copy the scaffold's budgets file. Then run `script/apply-settings` so the tenth check becomes required. Dispatch
   `mutation.yml` once on `main` before the release so the gate has a green full run to read.
   Door: two-way. Effort: S.

5. The weekly mutation verdict is red for a template reason, and it will block the release gate.
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

6. The methods a slotmap user reaches for first are missing.
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
   dies, and brings `top` to zero), each with a test and a README line. The spellings are the standard library's where it has one and the family's where it does not. The five names are ratified and ship in 0.1.0.
   Door: two-way until the tag; one-way after. Effort: S.

7. The walk bound and the live count have no induction step.
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

8. A performance figure with nothing to reproduce it.
   Finding: the README and the crate documentation state that an insert-and-remove cycle takes
   about 7 nanoseconds on a 2.1 gigahertz server core, which is how they argue that a slot can
   retire in about half a minute. The repository has no benchmark, so a reader cannot check the
   figure or re-measure it on another machine. Nothing measures the first-fit scan or a lookup.
   Evidence: `README.md` lines 104 to 105; `src/lib.rs` lines 14 to 16.
   Change: a `benches/` target with `harness = false` and no new dependency (`std::time::Instant` and `std::hint::black_box`). It measures insert-remove cycles at N of 16 and 4,096, a lookup, and the scan into a table that is nearly full. The README then states the figure with its method and the N it measured at. Or it drops the figure and keeps the arithmetic (2^32 cycles at the measured rate).
   Door: two-way. Effort: S.

### P2: later

9. The first-fit scan is O(N) on every insert.
   Finding: `insert_with` scans from slot 0 each time. A table of 4,096 that stays nearly full
   pays a few thousand compares per insert. Every other crate in the family keeps a free list; the
   README states the cost.
   Evidence: `src/lib.rs` lines 224 to 229.
   Change: measure first (item 8). If the scan shows, keep a lowest-free-slot cursor: insert scans
   from the cursor and moves it up past the slot it fills; remove lowers it to the freed slot when
   that is lower. First-fit order does not change, so the table mints the same names. The worst case stays O(N). The common case stops paying it. Four bytes of state.
   Door: two-way (no public behavior changes). Effort: M.

10. The storage layout has a measurement but no documentation and no comparison.
    Finding: generations and entries live in two arrays, so a lookup touches two cache lines. The insert scan reads two streams. `Table<u64, 256>` is 5,136 bytes and `Table<u32, 256>` or a
    table of references 3,088, which is the `Option` tag's cost on a `T` without a niche. The README says a tag costs space. It gives no formula and no figure.
    Evidence: `src/lib.rs` lines 166 to 180; the sizes above.
    Change: state the size in `BUGS` as `N * (size_of::<Option<T>>() + 4)` plus two words, with one
    measured example, and add a `size_of` test that pins it. Then, with item 8's benchmark in hand,
    try one array of `(u32, Option<T>)` and keep whichever measures better. Storage is private, so
    the layout is not a commitment.
    Door: two-way. Effort: S for the documentation, M for the comparison.

11. `EntryName` as a key in other collections.
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

12. The crate's front page on docs.rs shows no code.
    Finding: the crate-level documentation is prose and limits; the example lives in the README,
    which docs.rs does not render, and in `Table`'s documentation one click away. The documentation also omits two facts a user needs. `N` must be between 1 and `u32::MAX`, and the `const` assertion refuses `N == 0` at compile time. A large table built as a local sits on the stack, so it belongs in a `static` or must be built in place.
    Evidence: `src/lib.rs` lines 1 to 25 and 186; `README.md` line 50.
    Change: a ten-line example in the crate documentation, one sentence on `N`, one on the stack.
    Door: two-way. Effort: XS.

13. Four small text fixes for a stranger.
    Finding: the README's comparison never names `slab`. Most people meet the shape there (289M downloads in 90 days). It has no generation at all, so a stale index reaches the next occupant. The changelog's `Unreleased` lists the first version and then one addition to
    it, which at 0.1.0 is one line. CONTRIBUTING says the crate stays "free of vocabulary from any
    other tree", which means nothing to an outside reader; say that the `standalone` job refuses
    references to any other repository. CODEOWNERS says the maintainers' team may not exist; it
    does.
    Evidence: `README.md` lines 113 to 156; `CHANGELOG.md` lines 6 to 9; `CONTRIBUTING.md` line 17;
    `CODEOWNERS` lines 1 to 3.
    Change: one sentence on `slab` in the comparison; collapse the changelog entry at release;
    reword the CONTRIBUTING line; drop the CODEOWNERS caveat.
    Door: two-way. Effort: XS.

14. The `Zlib` allowance in `deny.toml` matches nothing.
    Finding: `cargo deny check` warns about the `Zlib` allowance. The crate has no dependency, so no allowance matches anything, and this is the one the tool reports. It is the
    scaffold's list, so it is harmless and a stranger running the check sees a warning.
    Evidence: `deny.toml` line 22.
    Change: none in this repository unless the template trims the list for a dependency-free crate;
    note it here so nobody chases it.
    Door: two-way. Effort: XS.

15. `rust-version` may be higher than the code needs.
    Finding: the manifest states 1.88. Edition 2024 needs 1.85, and nothing read in `src/lib.rs`
    visibly needs a later release (inline `const` blocks and `const { None }` array repeats are
    older). A lower stated minimum widens who can take the crate, and lowering it later is
    compatible while raising it is a change users feel. Not verified by a build.
    Evidence: `Cargo.toml` lines 4 to 5.
    Change: build and test at 1.85; if green, state 1.85 and let the weekly `msrv` job hold it.
    Door: two-way. Effort: XS.

