---
risk: 4
color: green
updated: 2026-10-05
---
# Appendix to risk 4: The architecture imposes a per-crossing cost that cannot be engineered away

*An appendix to [`4-the-per-crossing-cost.md`](4-the-per-crossing-cost.md) (risk 4). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim, and calef named this one first

A capability microkernel pays on every boundary crossing, and on workloads that cross constantly the
cost is architectural rather than a matter of tuning.

### The verdict of record

Superseded 2026-10-04 (UTC): the sweep ran on radon, five boots, and matched step 7's first outcome
in `notes/job-mix.md`. The risk 4 entry has the result and its caveats, and
[the evening's page](../../notes/job-mix/radon-2026-10-04.md) has the numbers. The paragraph below is
the 2026-09-23 state.

RUN, 2026-09-23. No verdict, and one bench evening stands between here and one. This is the
best-covered risk on the list by volume of measurement and it still has no answer, because
everything measured so far is a single crossing and the claim is about a cost that cannot be
amortized. Amortization is a property of a workload. The instrument that produces a workload number
is built, gated and rehearsed on three architectures, and no sweep from its current form has ever
run on silicon.

What is measured, and it is a lot.

- Single crossings, against Linux and macOS on one core. `notes/benchmarks.md`'s release table puts
  nife and Linux on the same M-series core at the same HVF tier, with native macOS as the bare-metal
  ceiling. Null syscall ~27 ns against Linux's ~139, IPC round trip ~337 ns against ~1,723, a
  derived context switch ~28 ns against ~415, and spawn ~7.7 us against `fork`+`exit`'s ~19.7 us.
  Provisioning a page is a three-way tie near ~550 ns, which the note keeps out of the win column
  because zeroing 4 KiB is bandwidth-bound and identical on all three. Four wins and one tie, each
  with its caveat stated where the number is: the tie is zeroing, the switch is derived by
  subtraction from two different mechanisms, and spawn builds a lighter object than `fork`
  duplicates.
- A committed floor per crossing, in counter ticks. `bench/baseline-aarch64.txt`: `ipc_rtt`
  1,032 ticks, `call_reply` 1,059, `relay_rtt` 2,058, `ctx_switch` 612, `spawn_reap` 3,383,
  `null_syscall` 20.5. Deterministic and comparable across runs, gated at a coarse 10%.
  *(Corrected 2026-09-30: this bullet said "in guest instructions"; one aarch64 bench tick is 16
  guest instructions under `-icount shift=0` (`CNTFRQ` 62.5 MHz), so the floors in instructions are
  the numbers times 16. The floors stand; the unit label was wrong. See
  [notes/benchmarks/icount-tick-scales.md](../../notes/benchmarks/icount-tick-scales.md).)*
- A bounded footprint for the crossing itself, which is the mechanism by which a per-crossing cost
  would fail to amortize in the first place. `bench/fastpath-aarch64.txt`, written by
  `script/fastpath-footprint`: `ipc_call_reply` 7,028 bytes plus `syscall_entry` 1,508. The first
  three phases of milestone 188 (the IPC fastpath) took entry from 3,304 to 1,508 and
  `ipc_send_receive` from 5,888 to 5,356. That work also found that the cheap extraction method of
  milestone 156 (extract the rest and ratchet both ways) does not transfer to a closure walk, which
  is a result rather than a shortfall.
- One real path closed against itself. Milestone 138 (close the read gap) measured 16x against where
  it started, including 5.67x on a read and 8.02x on a write from one wire change. And
  `call_reply`'s steady state is measured with the live-replacement mechanism costing zero in it,
  which is DECISIONS §41 (the endpoint is the broker).

Why none of that is a verdict, stated once so it is not re-litigated. Every figure above is a single
operation or a static size. Milestone 25 (cross-OS performance comparison)'s own block says its
suite is entirely single-operation primitives, *"the same shape DECISIONS §96 (process kernel or
event kernel) means by micro-benchmark"*. And milestone 168 (a multi-tasking workload benchmark)
exists because milestone 25 has that hole. A per-crossing cost a workload can absorb and one it
cannot look identical in these numbers.

The shape has been measured once on silicon and is not quotable. Five radon boots on 2026-09-16:
throughput rose to about four tasks and then plateaued through 32 without declining, on every boot,
at 8x oversubscription, with `tasks=1` repeating to 0.0% across five cold power cycles. That is the
shape a defense of this risk wants. It does not get to be one, for two reasons milestone 168 records
against itself. `tasks=4` spread 29.4% between boots and 37.3% within one, so a best-of-three there
is a coin flip rather than a number. And the mix contained no page mapping and no process creation,
which are the two jobs that go deepest into the kernel. The instrument was rewritten on 2026-09-19
(every point the median of 21 repeats, `map` and `spawn` jobs added). And no sweep from it has run
on a board. So every stability claim about the current instrument is a resampling of the old one's
data.

The decisive experiment has not been run, and it is now a bench evening rather than a lane:
milestone 168, one radon evening with the 2026-09-19 instrument, at least five boots, by
`notes/job-mix.md`'s procedure. Its step 7 wrote down what each outcome means before the numbers
exist, which is what keeps the reading from being a defense afterwards. Flat or rising through 32
with every point inside 10% across boots reads as no architectural per-crossing cost visible at this
scale on this silicon. a repeatable knee followed by a decline says a cost exists and grows with
load, with the `job-mix-kind:` lines naming which path pays. Anything still wider than 10% is not a
verdict and the spread gets recorded instead. Milestone 188 (the IPC fastpath) is the follow-on if
the path that pays is `round_trip`.

A cheaper cross-check became available on 2026-09-19 and nobody has taken it. calef asked that day
for the sweep under HVF on patagonia's own cores, as a cross-check on the shape and never as a
result. QEMU refused it five times because `kernel/src/drivers/gic.rs` was GICv2 only. 
Milestone 227 (a GICv3 driver) turned BUILT the same day, and its own block records that `script/job-mix --hvf`
was not yet on `main` when that lane ran, so the cross-check still has not happened although its
blocker is gone. It costs no board.

What was refused here, because the refusals are half of what makes the rest readable.

- Every x86 `ns/iter` in this tree. The 2026-08-24 table is marked suspect where it stands: the boot
  calibrated the TSC from one 10 ms PIT window, a poll can only notice the terminal count late, and
  200 boots put the worst error at +1153%, always high. Milestone 571 (the x86 boot calibrates the
  TSC once) fixed the estimator; it did not make the published figures quotable.
- Any nanosecond read off an `-icount` leg. Under `-icount shift=0,sleep=off` a guest nanosecond is
  a function of the instruction stream rather than of real time, and the implied rate moved 37%
  between two workloads inside one boot (`notes/tsc-under-tcg.md`).
- `sel4bench`. It builds and boots and has never produced a number, because it times one operation
  through `PMCCNTR_EL0` and neither TCG nor HVF provides one. This is the comparison the risk most
  wants and does not have: the cross-OS table's peer is Linux, not the state of the art in minimal
  kernels. argon was recorded in hand from 2026-09-01 (wrongly: a TK1 came and is going
  back, so the TX1 is awaited, 2026-10-06), milestone 74 (cycle counters) is what this side
  of the table needs to answer it, and nobody has run either half.
- The job mix's TCG rehearsals, which milestone 168 already refuses to record as results, since TCG
  models no cache.

What even a green answer would not cover, put here rather than after the run. Nothing in the mix
touches a disk, which is milestone 493 (a disk-file job mix needs a disk)'s territory, so a flat
curve is evidence about compute, memory, trap, scheduling, IPC, mapping and process creation, and
silent about the filesystem. The mix proportions are chosen rather than derived from an AIM7
workfile, so no figure from it is quotable without naming the mix. And a flat curve on four usable
harts says nothing about a machine with thirty-two.

The counter-thesis is published, and it is more specific than "microkernels are slow." Two papers
argue that the per-crossing cost is avoidable by removing the crossing. *The Case for Writing a
Kernel in Rust* (Levy, Campbell, Ghena, Pannuto, Dutta, Levis, APSys '17, DOI
10.1145/3124680.3124717) proposes to *"entirely throw away hardware protection within the kernel
and, instead, write the kernel in a memory-safe programming language"*. And RedLeaf (OSDI '20),
which builds a whole system on that bet. If they are right, a capability crossing is a cost this
project chose rather than inherited, and that is precisely what this risk says would be fatal.

Their own stated limits are the strongest thing in this entry's favor, and they are quoted rather
than asserted. The 2017 paper evaluates Rust *"in a single-threaded setting"* on low-power
uniprocessors. It leaves on-disk and in-hardware structures *"e.g. the page table"* to future work,
and says of information-flow control that *"it is not yet clear if such implementations would be
sound."* Their open problem is risk 5 below, this project's own hardest entry. Neither side gets to
treat multicore as settled.

What this obliges when milestone 168's number arrives. It will be read against theirs by anyone who
knows the literature. And a comparison is only honest if it says what differs. A language- isolated
crossing trusts the compiler and the absence of `unsafe` in the isolated code, where a capability
crossing trusts the hardware and a kernel small enough to prove things about. Those are different
guarantees at different prices, and the number alone does not say which was bought. This entry
carries the obligation; the reading of both papers is a lane's, and its note will be cited here once
it lands rather than promised from here.

### Ranked fourth on purpose

This is where a skeptic expects the project to die and it is where the project has the most evidence
that it will not. That evidence is the wrong shape, which is this entry's whole finding: a great
deal of it, all of it about one crossing at a time, and the claim is about many.
