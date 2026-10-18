# The first weekly runs: the 2026-09-03 sample and the 2026-09-14 census

This appendix of [notes/mutation-testing.md](../mutation-testing.md) holds the first two refreshes of
the 2026-08-03 baseline. The 2026-09-03 sample read a fall to 74.4%, then 83.4%, then 85.3%; the
2026-09-14 census found no fall at all.

## 2026-09-03: the weekly report had never run, and the first quarter to finish reads 74.4%

No weekly run had ever refreshed [the 2026-08-03 baseline](baseline-2026-08-03.md), and until
milestone 238 (two scheduled checks have never once succeeded, and one of them is a fatal risk's
refresh) nobody knew it. The `mutation testing` workflow failed all four of its scheduled runs, and a
scheduled workflow's red is an entry in the Actions tab with no badge. Milestone 232 (audit every
check against two questions: does anything run it, and does it block) found it. The diagnosis and
repairs are milestone 238's, and the workflow file carries them in full.

Two of the causes change what the parent note claims.

`--shard k/n` is zero-indexed, and the matrix ran `[1, 2, 3, 4]`. So `--shard 4/4` was an argument
error every week, and **`--shard 0/4` never ran at all**. With the old `slice` sharding that quarter
was an alphabetical block: `dtb`, `calendar`, `compositor`, `cred`, `elf`, `capability` and their
neighbors. Nothing was lost from the baseline, which was a full local run. But four weeks of reports
would have silently missed a nameable quarter had any of them succeeded.

The other cause is a single mutant allocating without bound, and the diagnosis took one wrong turn.
A resource sampler at 60 seconds showed 10 to 15 GB of memory free shortly before each kill. That
reads as runner eviction and was written up as such. At 10 seconds the same failure is 1.4 GB to
15.8 GB in twenty seconds, then the shutdown signal. The per-mutant timeout, auto-derived at 28 to
51 seconds here, cannot catch an allocation that finishes the machine in twenty. A sampler whose
interval is longer than the event reports innocence.

So this is not the `-j 2` bound [the baseline](baseline-2026-08-03.md) ran under, and lowering it
would not help: one runaway takes 14 GB by itself. The workflow now shards eight ways with
`--sharding round-robin`. That is damage control (a lost shard costs an eighth of every crate rather
than all of a few), not a repair. The repair is a bound on what one mutant may allocate, scoped as
its own work in milestone 238's block and built as [a memory bound per mutant](a-memory-bound-per-mutant.md).

### The numbers, and what each one is

Two runs on 2026-09-03, and the second is the one to read.

| | caught | missed | timeout | unviable | viable | killed |
|---|---|---|---|---|---|---|
| whole corpus, 2026-08-03 (baseline) | 4,654 | 391 | 96 | 410 | 5,141 | **92.4%** |
| shard 3 of 4, `slice`, 2026-09-03 | 1,710 | 598 | 27 | 127 | 2,335 | 74.4% |
| **shard 0 of 8, `round-robin`, 2026-09-03** | **910** | **187** | **32** | **97** | **1,129** | **83.4%** |

The round-robin row is the comparable one, which is why the sharding changed. It is a uniform
one-eighth sample of every mutant in the tree, so it covers all 60 crates and its rate estimates the
corpus rate.

The `slice` row is an alphabetical block, `nvme` through `work_steal_slot`. The crate is
`non_volatile_memory_express` since 2026-09-18. The old spelling stays because the shard boundary
was computed from it; the block would start elsewhere in the alphabet today. Thirteen of its
twenty-one crates did not exist at baseline. It is a rate for those crates, not for the tree, kept
only because it was the first shard this workflow ever completed.

So the tree's mutation score read as fallen from 92.4% to roughly 83.4% in a month. It is a sample,
not a census, and a second shard would move it. It is not a five-point question of sampling noise
either.

Corrected 2026-09-03 (milestone 244 (the largest crate in the tree is proved by nothing a mutation
can reach)), and the direction is up. Removing `system_initializer`, which no host test could ever
have reached, takes 25 uncatchable missed mutants out of the round-robin sample and 191 out of the
`slice` one. The rates are the same runs over a denominator that no longer counts mutants nothing
could kill:

| | caught | missed | timeout | unviable | viable | killed |
|---|---|---|---|---|---|---|
| shard 3 of 4, `slice`, corrected | 1,710 | 407 | 27 | 127 | 2,144 | 81.0% |
| **shard 0 of 8, `round-robin`, corrected** | **910** | **162** | **32** | **97** | **1,104** | **85.3%** |

So the drop is from 92.4% to roughly 85.3%, not 83.4%. Still a real fall, still a sample, still not
sampling noise. One of the three crates blamed for it was a bookkeeping gap, not an untested
surface. The two that remain (`uefi_loader` at 15%, `manual` at 52%) are the real ones.

### Where the drop is

The crates that existed at baseline are broadly stable or better: `gpt` 55/1, `elf` 12/0,
`calendar` 46/0, `glob` 14/0, `cred` 14/0, `dtb` 43/3, `filesystem_contract` 65/8, `grant_plan`
67/2. Three crates carry nearly all of the loss, and all three are new since the baseline:

- `system_initializer`: 0 caught, 25 missed in the sample (0 of 191 in the `slice` run, which saw
  all of them). Every mutant survives. Nothing in the host suite would notice any of its functions
  returning the wrong thing.

  Retracted on 2026-09-03 by milestone 244, which is what this bullet asked for. That crate was
  never in `.cargo/mutants.toml`, though the other three in its position are. That file's own head
  comment says its list mirrors `script/coverage`'s and asks the next person to keep the two in
  step. The crate reaches `user_mode_runtime`, so the host suite cannot compile a line of it. Both
  numbers above score a crate against a suite that could not have killed anything. It is excluded
  now, and `script/lint`'s bare-metal gate derives the four places that have to agree rather than
  asking anyone to keep them in step. The corrected rates are in the table above.
- `uefi_loader`: 3 caught, 17 missed. 15%.
- `manual`: 56 caught, 60 missed. 52%, and the crate is the documentation renderer.

This milestone deliberately did not act on that finding. `system_initializer` at zero was a
milestone of its own, not a line in a workflow repair.

It became milestone 244, and the answer was not the expected one. The lane measured the crate's 196
mutants by function before moving anything. 33 sit in pure logic and 157 in the syscall sequence,
about sixty lines of a 2,632-line file, so nothing was lifted and the block records why. What did
change the tree is the exclusion above and the gate behind it. See
`design/roadmap/0244-the-largest-crate-in-the-tree-is-proved-by-nothing.md`. Its most reusable line
is the method: **`cargo mutants --list -p <crate>` attributes every mutant to its enclosing
function.** So "where are this crate's mutants" is one command, worth running before any lane
proposes to restructure code for testability.

### What the sample does not say

It does not re-read `design/fatal-risks/README.md`'s third risk, which is an architect's. It is a sample,
not a census: the memory failure above killed seven of eight shards, so 8,700 of the 9,857 mutants
were still unrun since 2026-08-03. What it removes is the reason the stale number was acceptable,
the clause saying a refresh arrives on its own. A refresh had arrived, once, and it was lower.

## 2026-09-14: the first census since the baseline, and the fall was an artifact

**The workflow succeeded, all eight shards, for the first time since it was written.** Run
[34833498873](https://github.com/nifeos/nife/actions/runs/34833498873), scheduled, took 54
minutes. It covered 10,012 mutants over 64 crates: 8,303 caught, 771 missed, 203 timed out, 735
unviable. That is **91.7% of viable mutants killed**. Milestone 277 (bound what one mutant may
allocate, so a runaway kills the mutant and not the machine) is what made it finish. The runaway
that had taken seven of eight shards every week did not take one.

This is the re-run `design/fatal-risks/README.md`'s third risk had been waiting for since 2026-08-03, and it
is a census rather than a sample.

### The like-for-like number is up, not down

The 2026-09-03 section read the score as fallen from 92.4% to roughly 85.3%. A full run shows that
is wrong. The 38 crates that existed at baseline scored **93.6%**, against 92.4% for the same 38
crates a month earlier.

| | crates | caught | missed | timeout | viable | killed |
|---|---|---|---|---|---|---|
| whole corpus, 2026-08-03 (baseline) | 38 | 4,654 | 391 | 96 | 5,141 | **92.4%** |
| the same 38 crates, 2026-09-14 | 38 | 6,008 | 417 | 127 | 6,552 | **93.6%** |
| whole corpus, 2026-09-14 | 64 | 8,303 | 771 | 203 | 9,277 | **91.7%** |

So there was no fall. The 85.3% was a one-eighth sample taken while two crates were scored against
test suites that could not run. One was `uefi_loader`'s `[[bin]]` half, whose 154 mutants came back
missed in "0s build + 0s test" because nothing rebuilt ([the uefi_loader appendix](uefi-loader.md)).
The other was `documentation` (then `manual`), measured with a third of its suite compiled away
behind a default-off Cargo feature ([the documentation appendix](documentation.md)). Milestone 280
(`uefi_loader` at 15% and `documentation` at 52% are unexplained holes in the published score)
fixed both. The two crates blamed for the drop now score: `uefi_loader` 100%, `documentation` 95.4%.
(Corrected 2026-09-24: the 2026-09-21 census, run 35589550926, measured `uefi_loader` at 48.9% of
356 viable, 182 missed, after 34 viable at 100% on 2026-09-19. `documentation` held at 95.4%.)

The gap between 93.6% and 91.7% is the 26 crates that did not exist at baseline. New crates arrive
less well tested than old ones, which a month of lanes should be expected to produce. It is a
worklist rather than a verdict.

### Two things a census shows that a sample cannot

Three of the baseline's five perfect crates lost their perfect score. No sampled run would have
surfaced that as a regression, because a sample cannot distinguish an absent mutant from a killed
one. These are the survivors to triage first, because each is a property that held at the baseline (see
[the 2026-09-19 triage](regressions-2026-09-14-capability-to-device-tree-blob.md)):

| crate | baseline | 2026-09-14 | missed |
|---|---|---|---|
| `memory_regions` | 100.0% | 88.9% | 0 -> 8 |
| `capability` | 97.4% | 88.2% | 1 -> 8 |
| `clock_contract` | 96.8% | 91.0% | 2 -> 6 |
| `elf` | 100.0% | 94.2% | 0 -> 6 |
| `swish` | 94.6% | 89.4% | 3 -> 20 |
| `dtb` | 99.7% | 96.6% | 1 -> 14 |
| `filesystem_contract` | 93.1% | 90.1% | 37 -> 59 |

`nifefs`, `dma_validator` and `bitmap_font` held at 100%. The largest gains are real too, mostly
where a triage pass was spent: `intrusive_fifo` 57.1% to 100%, `line_editor` 80.1% to 98.5%, `pci`
77.3% to 90.7%, `ipc` 82.8% to 96.6%, `glob` 89.2% to 100%, `user_mode_heap` 89.3% to 100%.

And the eight worst crates in the tree are all new, none of them in the baseline:

| crate | killed | missed of viable |
|---|---|---|
| `work_steal_slot` | 54.2% | 11 of 24 |
| `memory_corruption_canary_gate` | 66.7% | 6 of 18 |
| `soak_page` | 68.2% | 7 of 22 |
| `timetable` | 73.6% | 48 of 182 |
| `jh7110_entropy` | 76.8% | 19 of 82 |
| `mdns_proto` | 77.3% | 82 of 362 |
| `job_mix` | 77.8% | 6 of 27 |
| `schedule_store` | 78.4% | 8 of 37 |

Read `timetable` first, and not because of its rate. It holds `next_after`, the property
`design/fatal-risks/README.md`'s risk 2 names as its strongest counterfactual. That is the milestone 6
(threads, the context switch, and preemption) timer drift, proved in this tree over code the timer
does not call. 48 survivors in a crate carrying a proof is the shape risk 2 is about.

### Two caveats on the comparison, neither of which moves the reading

The run predates the rename in milestone 265 (`_proto` is a truncation, and it collides with the other
word it could be short for) by seven hours. The sweep started 10:30 UTC on 2026-09-14; #860 merged
at 17:12. So its artifacts spell the protocol crates `*_proto`, and the baseline file spells them
`*_protocol`. Every table above maps the nine affected names; nothing else about them differs. The
next run's artifacts will match the baseline's spelling without help.

The baseline's `caught` was derived, and this run's is reported. The head comment on
`.cargo/mutants-baseline.txt` records that the 2026-08-03 run was resumed twice. So its caught column
is total-minus-the-rest, and three mutants could not be attributed at all. This run was a single
clean pass per shard. That difference favors the baseline being slightly generous, which makes the
+1.2 like-for-like gain a floor rather than a ceiling.

The baseline file was deliberately not updated. Replacing it would destroy the comparison the next
run wants, and choosing when a new census becomes the baseline is not a records edit.

### What the census does not say

It does not re-read `design/fatal-risks/README.md`'s third risk. That is an architect's, and
`design/roadmap/proposals/fatal-risk-3-against-the-new-number.md` has been holding it since
2026-09-03. The census gives that proposal the number it was written to be read against, and the
number is not the one anyone expected.

It says nothing about the kernel or the arch trees, which are excluded by construction and are where
risks 5 and 9 live. **Mutation testing measures the test suite, not the code.** One green census does
not make a habit: this was the first weekly run to finish, and the cadence wants a second data point
before anyone quotes a trend.
