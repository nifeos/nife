# The 2026-09-14 regressions: `capability` to `dtb`

*Name: ratified 2026-10-18 (calef, "Approve all"). Refused `regressions-capability-to-dtb`.*

The first half of milestone 326 (nobody has been assigned to turn a mutation score upward)'s
triage of the crates that regressed at the 2026-09-14 census. It verifies the per-crate figures
that [notes/mutation-testing.md](../mutation-testing.md) summarizes. The second half is
[regressions-2026-09-14-clock-contract-swish-filesystem-contract](regressions-2026-09-14-clock-contract-swish-filesystem-contract.md).

## 2026-09-19: milestone 326, triaging the census's regressions

The census of 2026-09-14 produced 771 survivors, and nobody had looked at one.
`design/fatal-risks/README.md`'s risk 3 is AMBER for exactly that reason. Its stated condition for going
back to green is that milestone 326's first two parts carry no untriaged survivor. This section is
that accounting, crate by crate, in the block's own order.

Each crate was re-derived with `script/mutation -p <crate>`, not read out of the census artifacts,
per that block's BUGS section: the identities are not in the tree, only the counts. Every kill here
was verified by re-running the sweep. That is the discipline
[the baseline ledger](baseline-ledger.md) argues for ("a verdict reached by reading is wrong about
ten percent of the time; a verdict reached by running is not"). So is every equivalence claim: the
three mutants called equivalent below are the three the second run still reports.

### `capability`: 8 survivors, 5 killed, 3 equivalent

Before: 60 caught, 8 missed, 16 unviable (88.2% of viable). After: 65 caught, 3 missed (95.6%).
The baseline's single survivor here was the `1 << 0` degenerate case, still present and still
equivalent. Everything else arrived with milestone 126 (the `procps` package)'s `SURVEY` predicate
and milestone 231 (nothing counts how many capability slots a boot actually uses)'s high-water mark.

**`survey_includes` had no `cargo test` caller at all.** Replacing the whole function with a
constant `true` survived, and so did a constant `false`. The predicate decides which threads a
supervision rendezvous may see (milestone 126). calef ruled on 2026-08-17 that a domain names its
members and never acts on them. That is why it is a separate right rather than a corner of `READ`.
Two Kani harnesses prove it for every input, so this is a gap in the suite rather than in the code.
But `script/verify` is a different gate on a different cadence. A build answering "everyone is in
your domain" would have reached a reviewer with `script/test` green. Closed by
`a_survey_shows_this_rendezvous_children_and_nobody_else`: four cases, against the four the reap
gate already had.

Three mutants in the packed high-water word survived a floor.
`the_global_high_water_mark_is_never_below_a_table_that_reached_it` asserts only
`peak >= cs.peak()`, deliberately, because `PEAK` is one static shared by every `CapabilityTable` in
the binary. A floor of five does not notice `>>` becoming `<<` in the peak half. Nor does it notice
`&` becoming `|` or `^` in the ceiling half, since each still yields a number above five. Closed by
`the_high_water_word_reports_the_occupancy_and_the_capacity_that_set_it`. It buys exactness back
from a shared static by setting a record no other test in the module can reach. `fetch_max` only
raises, so 40 of 64 is the standing record whatever order the harness runs in.

The three equivalents, argued from the code and confirmed by the second run:

- `Rights::READ`'s `1 << 0` under `1 >> 0` (1). Both are 1. This is the degenerate case
  [the recurring patterns](baseline-2026-08-03.md#patterns-that-recur-named-once) name. It is
  recorded rather than excluded, so it stays visible if the constant ever moves off zero.
- `grew`'s `self.used > self.peak` under `>=` (1). The extra branch fires only when `used` already
  equals `peak`. There the assignment `self.peak = self.used` is a no-op. The `note_peak(self.peak, N)`
  beside it re-offers a packed word this table has already offered. `peak` is assigned only on that
  line, and `note_peak` is called in the same breath every time, so any state with `used == peak`
  has already published that pair. `fetch_max` is idempotent. The mutant differs by one relaxed
  atomic operation and nothing observable.
- `note_peak`'s `((peak as u32) << 16) | ceiling` under `^` (1). The two operands share no bit. The
  shift clears the low sixteen bits. The line above clamps `ceiling` with
  `min(ceiling, u16::MAX as usize)`, so it cannot reach the high ones. On disjoint bits `|` and `^`
  are the same function. The clamp is load-bearing for this claim, and it is itself a caught mutant.

### `memory_regions`: 8 survivors, 6 killed, 2 excluded

Before: 64 caught, 8 missed, 2 unviable (88.9% of viable). After: 70 caught, 0 missed (100.0%).
That is back to the baseline's perfect score. The two exclusions are a category rather than this
crate's business.

`has_children` had four survivors, one of them the whole function replaced by `false`. The reason
is a shape rather than an oversight. `claim_for_destroy` reads the `children` count through
`destroy_outcome`, not through this accessor. So `a_parent_refuses_until_its_last_child_returns`
proves the refusal without ever calling the predicate the kernel asks first. The one test that did
call it, `a_dead_name_is_inert_everywhere`, called it on a dead name. There `get` returns `None`
and the closure inside is never evaluated. That is why `children > 0` under `==`, `<` and `>=` all
survived beside the constant. Closed by `has_children_answers_for_the_living_the_childless_and_the_dead`,
which asks it of a fresh root, a root with a child, a leaf, and a dead name.

`retype_object_page`'s arithmetic had two: `base_page + watermark` under `-`, and `watermark += 1`
under `*=`. Both also appear in `retype_page`, where both are caught, and the split is the whole
explanation. Every existing test calls the object retype exactly once, at watermark zero. There the
two arithmetics agree, and a watermark that never advances is invisible. A page handed out twice is
two kernel objects on one frame. Closed by `the_object_retype_walks_the_region_one_page_at_a_time`.
It takes two pages from a non-zero base, then exhausts the region through the other entry point to
show the budget is shared.

The two remaining are the loom model, excluded as a class. `Reached::mark` and `Reached::assert`
are the non-vacuity flags described in this crate's verification section. They sit in
`mod interleavings`, which is `#[cfg(all(test, loom))]`. `cargo test` never compiles it, so a
mutant there always survives. That is exactly why `verification::` and `proofs::` are already
excluded, and the entry added to `.cargo/mutants.toml` says so. It covers all five loom models in
the tree (`clock_contract`, `memory_corruption_canary_gate`, `memory_regions`,
`thread_wake_handshake`, `work_steal_slot`), every one behind the same `cfg`. Their checker is
`script/interleaving-check`; excluding them here does not make them proved.

### `elf`: 6 survivors, 6 killed

Before: 98 caught, 6 missed, 21 unviable (94.2% of viable). After: 104 caught, 0 missed (100.0%).
That is back to the baseline's perfect score. All six sat in the machine-check machinery that
milestone 288 (host tests that assume the host is aarch64) added on 2026-09-14, which is why the
fall is recent.

**All six are one mistake in two functions: a test that proves a refusal rather than the thing
being refused.** `a_binary_for_the_other_supported_machine_is_refused` iterates `FOREIGN_MACHINES`
and expects each entry turned away. Milestone 288 wrote it in exactly that shape, and any wrong
number satisfies it. So replacing the whole derivation with `[0; _]` survived, and so did `[1; _]`.
An array naming no architecture at all would have gone on passing a symmetry test while proving
nothing about symmetry. It is `gpt`'s lesson from [the calibration](baseline-2026-08-03.md)
(rejection is not rejection for the right reason), in a crate nobody expected it in. Closed by
`foreign_machines_is_every_known_machine_but_this_builds`, which asserts the membership rather than
the consequence: every known machine is in exactly one of native and foreign.

`machine_no_nife_build_accepts` had the mirror pair, plus its loop bound. The function hands back
the machine number it was given, having proved no nife build accepts it. It exists because a test
naming `EM_X86_64` as foreign stopped being true the day `x86_64` became a target (milestone 161
(the x86_64 kernel port)). Returning a constant instead survived for the same reason as above. Its
loop bound `i < KNOWN_MACHINES.len()` survived under `==` and under `>`. Both never enter the loop,
so they never check anything. No test could see it, because no test ever gave the guard a machine it
should reject, and a guard is only tested by tripping it. Closed by two tests:
`a_machine_no_build_accepts_is_handed_back_unchanged` for the identity, and
`a_machine_this_tree_runs_on_is_refused_by_the_guard`, a `should_panic` that trips it with
`NATIVE_MACHINE`. The runtime panic is the same assertion a `const` context turns into a build
error, which is what the guard is for.

### `timetable`: 48 survivors, 12 killed, 36 excluded, and `next_after` was not among them

Before: 134 caught, 48 missed, 12 unviable (73.6% of viable). After: 146 caught, 0 missed
(100.0%).

The headline first, because the block asked for it. `design/fatal-risks/README.md`'s risk 2 names
`next_after` as its strongest counterfactual: the milestone 6 (threads, the context switch, and
preemption) timer drift, proved over code the timer does not call. Milestone 326 put this crate
second on its list for that reason, not for its rate. **No survivor touched `next_after`, the phase
arithmetic, or the firing decision.** Every mutant in that function was caught before this lane
touched anything, by `next_after_is_strictly_in_the_future_and_keeps_its_phase` and the doctest
beside it. That is evidence against risk 2 in the one place the roadmap thought it most likely.

Thirty-six of the 48 were the crate's own Kani harnesses, which is a defect in the instrument.
`.cargo/mutants.toml` excludes `verification::` and `proofs::` as module paths, because
`cargo test` never compiles a `#[cfg(kani)]` harness, so it always "survives". `timetable` puts its
harnesses in `src/proofs.rs`, not in an inline `mod proofs { .. }`. cargo-mutants names a mutant by
the function's path within the file it parsed. The inline form yields
`proofs::a_fire_is_strictly_in_the_future`; the file-per-module form yields
`a_fire_is_strictly_in_the_future` with no prefix, and the regex misses it. So this crate's 73.6%
counted its own proof file against it: arithmetic, not a finding. The same family includes
`system_initializer`, in milestone 244 (the largest crate in the tree is proved by nothing a
mutation can reach). It also includes `uefi_loader`'s `[[bin]]` half, in milestone 280
(`uefi_loader` at 15% and `documentation` at 52% are unexplained holes in the published score). Closed with `**/src/proofs.rs` and
`**/src/verification.rs` in `exclude_globs`. A glob, not a regex, because the file layout is what
differs. `timetable` is the only crate with that layout today. The sibling glob is there so the
convention cannot arrive unexcluded.

The twelve real survivors sat in the parser, the admission check and the plan writer:

- `Error::line` replaced by a constant `1` (1). `each_error_points_at_the_line_that_is_wrong`
  compares whole `Error` values, so the line is checked but the accessor is never called. It is
  what sends a person to the fault. Closed by `error_line_reads_the_number_each_variant_carries`.
- The plan writer's millisecond branch (3), which no test ever rendered. Every plan in the suite
  used seconds or minutes. So `nanos / (NANOS_PER_SEC / 1000)` could become `%`, and either `/`
  could become `*`, with nothing to see it.
- The plan writer's length arithmetic (3). `write_schedule` fills a 32-byte space-filled buffer
  and emits `buf[..n.max(SCHEDULE_COLUMN)]`. For any schedule inside twelve columns the returned
  length never reaches the output. So three mutants were invisible against `30s` and `1m`:
  `6 + write_interval(..)` under `-`, and `n += unit.len()` under `-=` and under `*=`. A
  seven-digit interval pushes past the column. There the first underflows, the second truncates the
  text and the third pads it.
- `e.mem_pages > 0` under `>=` and `e.arg != 0` under `==` (2). Both were only ever exercised
  true, so nothing proved the lines are omitted for an entry that asks for neither. All eight
  mutants above are closed by `the_plan_prints_milliseconds_long_intervals_and_the_absent_grants`.
- `admit`'s `Holdings { dir: held.dir, .. }` with the field deleted (1), falling back to the
  default `false`. A scheduler that holds a directory would have reported every `rm` line as
  unbackable. Closed by `a_designation_is_backed_by_the_directory_the_scheduler_holds`.
- Both `!` in `unbacked`'s `!held.dir` tests (2), which have a finding under them. They survived
  because `admit` cannot reach either branch. It hands `grant_plan::plan` the same `dir` bit that
  `unbacked` then re-tests. So a plan needing a directory the scheduler lacks is refused during
  planning, and never reaches the check that would name it unbacked. `Unbacked::File` has a second
  reason: no shipped program declares a `FileSpec::Required`, so `Endowment::file` is `None` for
  every plan this crate can build. The mutants are killed by calling `unbacked` directly. The
  reachability is recorded in a `BUGS` section on `Unbacked` itself, where a reader meets the
  variants. Whether `admit` should stop pre-consuming the holding is a behavior change, not a test,
  so it is recorded and not made.

### `dtb`: 14 survivors, 13 killed, 1 equivalent; the 29 timeouts are hangs

Before: 368 caught, 14 missed, 29 timeouts, 19 unviable (96.6% of viable). After: 381 caught, 1
missed (99.7%), the baseline's own number.

Thirteen of the fourteen are one omission repeated across four walkers. `Dtb` has five walks over
the structure block, each keeping its own 16-entry per-depth array.
`a_declaration_at_the_stacks_edge_is_ignored_not_indexed`, from milestone 42 (supply chain and
fuzzing in CI)'s fuzzing leg, tests exactly one of them: `node_reg`'s. `node_prop_compatible`,
`node_prop_inherited`, `phandle_prop` and `node_prop` carry the same `depth < MAX_DEPTH` guard, and
`<=` survived in every one. At depth exactly 16 that is an out-of-bounds index. It sits in a parser
the kernel runs on firmware bytes, before there is any way to report a failure. Three of them also
had the guard alive under `==` and `>`. Those skip the per-node reset at every depth a real tree
reaches, so a sibling answers with the property of the node that closed before it.

Closed by three tests in `tests/hostile.rs`:

- `every_walkers_stack_edge_is_ignored_rather_than_indexed`;
- `a_sibling_does_not_answer_with_its_predecessors_property`;
- `an_inherited_property_comes_from_the_named_node_not_the_first_one`. It also covers
  `node_prop_inherited`'s target guard under `||`: `(A && B) || target_at.is_none()` selects the
  root, whose slot then answers for a node the tree does not contain.

The sweep also audited the tests. The first version of two of those tests passed and killed
nothing. `node_prop_compatible` takes `(compat, name)`, and they were written `(name, compat)`: a
vacuous assertion that reads correctly. The re-run said so, in the only way that is not an
argument. This is the ledger's rule about reading and running, applied to the kill rather than to
the equivalence.

The one equivalent is `cells`'s `value = (value << 32) | be32(..)` under `^`. The shift clears the
low thirty-two bits, and a `be32` cannot reach the high ones. So the operands are disjoint, and `|`
and `^` are the same function on them: the argument for `capability::note_peak` above.

The 29 timeouts are hangs: the baseline's `dtb` row, grown with the walkers. All 29 are `at += 4`
under `-=`, `at += align4(..)` under `-=`, or `at = value_at + align4(len)` under `-`, spread over
the nine walks. Every one is the structure-block cursor, and a cursor that stops advancing re-reads
the same token forever. That is the tests noticing rather than missing, which is how
[the main page](../mutation-testing.md#how-to-read-the-numbers) reads a timeout whose mutant could
hang.
