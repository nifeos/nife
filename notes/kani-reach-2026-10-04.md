# Does a standing proof notice a regression?

Milestone 741 (does a standing proof notice a regression), for fatal risk 2 (the proofs prove
trivia, and the real bugs live where Kani cannot reach). calef asked for it on 2026-10-04 (UTC).
Fatal risk 2's amber has a red half: no standing proof has caught a regression, and every catch in
milestone 191 (did the proofs catch the bugs?) came while a harness was being written. The defects
that escaped most recently were tests and gates that could not fail. This note asks whether the
proofs belong to the same class, and answers by mutating the code they cover.

*Note name provisional, per the naming tenet. An architect names things.*

## The finding

**Most standing proofs can fail.** 189 harnesses were measured. 184 of them reach at least one
viable mutant, and 178 of those 184 go red on at least one. Taken together, the proofs kill 1,668
of the 2,626 viable mutants in their reach (64%). The tests in the 2026-10-03 census catch 2,316 of
the 2,540 that census scored (91%).

**Six harnesses reach mutants and kill none.** Four are panic-freedom or totality proofs, and
cargo-mutants rarely makes a function panic; it makes a function wrong:

| package | harness | direct | transitive |
|---|---|---|---|
| device_tree_blob | `be32_is_total` | 0/2 | 0/2 |
| device_tree_blob | `be64_is_total` | 0/2 | 0/2 |
| elf | `check_segment_bounds_never_panics` | 0/8 | 0/8 |
| elf | `note::note_extent_never_panics` | 0/6 | 0/14 |
| machine_discovery | `riscv64::a_multi_letter_extension_is_never_read_as_a_privilege_letter` | 0/13 | 0/133 |
| pci | `ecam_offset_stays_inside_the_window` | 0/9 | 0/9 |

That does not make the six worthless. A panic-freedom proof guards against new code that panics,
and no mutant here writes new code. But no regression of the kind measured here could turn any of
the six red. For the two elf proofs, a sibling harness does kill some of the same mutants.

**Five harnesses reach no mutant at all, so this method cannot score them.** Four are
`inter_process_communication` proofs over `unsafe fn`s, which cargo-mutants never mutates (BUGS in
`helpers/kani_reach.py`). The fifth is pci's `a_fresh_bus_queue_holds_the_root_and_nothing_else`.

Proofs are rarely the only thing that notices. 17 mutants are killed by a proof and survive
every test. Another 29 are killed by a proof where the tests only hang (census `timeout`). 162
mutants in a proof's reach are killed by neither. Some of those 162 may be equivalent mutants;
none was triaged.

## Per package

"In reach" means some harness's computed transitive reach includes the mutated function. "Tests"
counts census `caught` against census-scored mutants. "Proof only" means a proof killed the mutant
and the census recorded `missed` or `timeout`. "Neither" means no proof killed it and the census
recorded `missed`.

| package | harnesses | in reach, viable | proof killed | no verdict | tests caught | proof only | neither | Kani CPU min |
|---|---|---|---|---|---|---|---|---|
| address_space_identifier | 3 | 23 | 20 | 0 | 21 | 0 | 2 | 0 |
| capability | 14 | 76 | 54 | 0 | 69 | 4 | 2 | 9 |
| component_plan | 5 | 102 | 64 | 0 | 84 | 6 | 10 | 20 |
| credential_protocol | 3 | 52 | 43 | 0 | 50 | 0 | 2 | 1 |
| device_tree_blob | 4 | 8 | 6 | 0 | 8 | 0 | 0 | 0 |
| direct_memory_access_validator | 7 | 83 | 54 | 3 | 79 | 0 | 0 | 128 |
| elf | 8 | 95 | 40 | 3 | 82 | 4 | 9 | 112 |
| filesystem_protocol | 3 | 20 | 7 | 0 | 11 | 0 | 6 | 1 |
| generational_table | 4 | 40 | 34 | 0 | 35 | 0 | 5 | 4 |
| globally_unique_identifier_partition_table | 9 | 263 | 205 | 1 | 259 | 0 | 3 | 113 |
| inter_process_communication | 14 | 36 | 26 | 0 | 30 | 4 | 2 | 2 |
| intrusive_fifo | 1 | 7 | 7 | 0 | 7 | 0 | 0 | 1 |
| jh7110_entropy | 4 | 20 | 17 | 0 | 19 | 0 | 1 | 2 |
| kernel (aarch64 and `system_call`) | 5 | 80 | 51 | 0 | no census | 0 | no census | 5 |
| machine_discovery | 16 | 403 | 218 | 0 | 380 | 9 | 14 | 1066 |
| manifest_note | 2 | 80 | 49 | 0 | 69 | 4 | 7 | 159 |
| memory_regions | 4 | 20 | 20 | 0 | 17 | 0 | 0 | 0 |
| network_time_protocol | 7 | 129 | 103 | 0 | 121 | 0 | 7 | 7 |
| nifefs | 2 | 47 | 12 | 0 | 47 | 0 | 0 | 4 |
| non_volatile_memory_express | 8 | 102 | 70 | 0 | 100 | 0 | 2 | 3 |
| package_archive | 2 | 69 | 20 | 0 | 58 | 1 | 8 | 8 |
| page_frames | 5 | 50 | 38 | 0 | 48 | 1 | 1 | 1 |
| paging | 36 | 422 | 324 | 0 | 378 | 0 | 44 | 8 |
| pci | 8 | 162 | 43 | 0 | 142 | 7 | 13 | 8 |
| subtree_scope | 5 | 48 | 21 | 0 | 42 | 1 | 2 | 97 |
| timetable | 10 | 189 | 122 | 0 | 160 | 5 | 22 | 39 |

The per-harness table, with killed/total for direct and for transitive reach, is
[`kani-reach-2026-10-04.csv`](kani-reach-2026-10-04.csv). Besides the six above, 30 harnesses with at
least four mutants in reach kill a quarter or less of them. The weakest by ratio are
`direct_memory_access_validator`'s `an_oversized_batch_is_refused` (3/75 transitive),
`package_archive`'s `a_short_file_is_refused` (3/65) and `paging`'s
`domain::the_grant_enumeration_is_total` (1/11). These are refusal and totality properties. A low
ratio is expected there and is not a defect in itself.

## Method

`helpers/kani_reach.py` (name provisional) has three steps, run by
`.github/workflows/kani-reach.yml` (dispatch only):

1. plan lists cargo-mutants 27.1.0's mutants for one package. It uses `--no-config` with the
   same harness and test-module exclusions as `.cargo/mutants.toml`, so `kernel` can be listed. It
   parses every `#[kani::proof]` and computes reach as a name-based call graph over
   comment-stripped source, resolving a name in the caller's own file first. Direct is the
   harness body plus helpers in its own proof module. Transitive is the closure over every `fn`
   in the package.
2. run applies one mutant at a time from cargo-mutants' own diff, runs `cargo kani -p <pkg> -j 1`,
   and restores the file from the bytes it read. A harness kills a mutant only when it ends
   `FAILED` and names a failed check. A harness timeout (`--harness-timeout`, at least 60 s and at
   least three times the harness's green time) or a CBMC out of memory is no verdict. It is never
   counted as a kill. A mutant that does not compile under Kani is unviable and leaves every
   denominator. Every run sits under a 12 GiB address-space ceiling.
3. report joins the 2026-10-03 census (run 37108924347). It pairs mutants by file and
   description: on the same line where one exists, otherwise in order, since lines moved between
   the two trees. 6 mutants outside the kernel could not be paired and are left out of the test
   columns.

## Runs and cost

| run | what | result |
|---|---|---|
| 37168513701 | pilot: nifefs and elf, every mutant against every harness; kernel, reached mutants only | 1h03m wall |
| 37178899420 | first scaled run, 41 shards at once | cancelled by the maintainer: it queued 40 jobs on the org's shared runners |
| 37179003058 | scaled run, `--reached-only`, `max-parallel: 4` | about 8 h wall; 39 of 42 shards green |
| 37219544157 | the three lost shards, after the fixes below | green |

30.0 Kani CPU-hours in total. That is the sum of `cargo kani` wall time across every mutant and
baseline, including elf's 1.9 h of proving every mutant against every harness in the pilot. The cap
of four concurrent shards is recorded in the workflow: wall clock is the cheap side of that trade,
because nobody waits on this run.

The scaled run lost three shards, all to the experiment rather than to the code under it:

- capability and pci: a helper `fn` nested inside a harness body was taken for a harness, Kani
  refused to match it, and the baseline never went green. The detector now requires the
  attribute to sit directly on the `fn`.
- subtree_scope 1/2: after 93 minutes the runner lost communication with the server, with no
  step marked failed. That is the memory-exhaustion signature the mutation census met; the
  12 GiB ceiling came from it.

glob and calendar are not measured. With `--reached-only` their cost was estimated at 53 and
38 CPU-hours, against 29 for everything else. The estimate is serial proof time from
`script/verify`'s table times reaching harnesses, plus 15 s of compile per mutant; for elf it gave
3.4 h against 3.6 h measured. Also not measured are the kernel's riscv64 and x86_64 harnesses
(the runner proves its own `arch/`) and `components`.

## Caveats

- The reach is approximate in both directions. Shared names (`new`, `len`) over-count it. A
  call not spelled as a name under-counts it: in the pilot, 8 of elf's kills fell outside the
  computed reach, through `Iterator::next` under a `for` loop and through `u32le`. Every package
  after the pilot ran `--reached-only`, so such kills went unseen and those scores are lower
  bounds.
- Bounded reach is the proof's, not the mutant's. A harness that `kani::assume`s the input
  away from the mutated path, or bounds a loop below it, cannot kill that mutant. That is the
  harness's real reach, and it is counted as a miss.
- The kill rule tightened partway. The pilot and 38 scaled shards counted any `FAILED` as a
  kill. The three rerun shards required a named failed check and logged zero `FAILED`s without one
  and zero out-of-memory outcomes. Earlier shards cannot be rechecked, because their logs were not
  kept.
- Kani 0.68.0 and CBMC 6.11.0 in CI, installed unpinned exactly as `verify.yml` does.
  patagonia has 0.67.0.
- Census-unviable mutants that compile under Kani (11) are scored, because the two builds
  differ in `cfg` (`kani` against `test`).
- cargo-mutants never mutates an `unsafe fn`. A proof over one is invisible to this method and
  to the weekly census alike.

## Follow-ups

- **Proposed.** Measure glob and calendar at their estimated 90 CPU-hours, two or three shards a
  week at `max-parallel: 4`:
  `design/roadmap/0782-measure-the-reach-of-the-glob-and-calendar-proofs.md`.
- **Proposed.** Decide each of the six zero-kill harnesses: give it a property a wrong value can
  break, or record why panic-freedom alone is the point:
  `design/roadmap/0780-give-the-zero-kill-harnesses-a-property-a-wrong-value-breaks.md`.
- **Proposed.** A reach check beside the weekly mutation census that fails when a harness's
  kill count drops to zero:
  `design/roadmap/0776-a-weekly-check-that-every-proof-can-still-fail.md`.
- **Recorded.** The `unsafe fn` blind spot and the reach approximation, in `helpers/kani_reach.py`'s
  BUGS.
