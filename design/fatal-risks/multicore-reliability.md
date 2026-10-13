---
risk: 5
color: none
updated: 2026-08-31
---
# Appendix to risk 5: It cannot be made reliable on multicore, and the bugs appear only on silicon

*An appendix to [`5-not-reliable-on-multicore.md`](5-not-reliable-on-multicore.md) (risk 5). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim

The concurrency is wrong in ways that QEMU cannot show and that arrive one at a time, forever.

### The verdict of record

RUN, 2026-09-25, on radon only: 8 h 09 m, 4.1 million cross-core handoffs, zero refused wakes,
wrong replies or stalls (`notes/visionfive2.md`, "The eight-hour soak, 2026-09-25"). Still no
verdict: one boot is one draw, and argon and xenon have not run it. What follows is the record as
of 2026-09-23.

NOT-RUN, 2026-09-23. No verdict, and the reason no verdict is available is itself the finding. This
entry carried the provisional word `UNRUN` for the day between its own revision and calef's ruling.
And said at the time that this file's vocabulary was `RUN`, `MEASURED` and `AUDITED`, which was
never what `script/fatal-risks` implemented. The ratified vocabulary is three words and `NOT-RUN` is
one of them, for exactly the case that sentence could not name. Until this date the entry carried no
status at all and cited exactly one milestone where the other eight cite between four and twelve.
And `script/fatal-risks`' own report is where that showed up. What the sweep behind this revision
found is that the single citation was the smaller half of the problem: the sentence this entry
opened with had been retracted in the tree two weeks before this file was written.

### The one failure on record is not a failure, and this tree overturned it itself

This entry used to say the risk had already fired. And that the VisionFive 2 had produced a receiver
woken with nothing delivered on three harts that no emulator run had ever shown. That is
`notes/visionfive2.md`'s fourth bench stop (boots 7 and 8, 2026-08-14). And the same note's fifth
bench stop overturned it on 2026-08-15: the dumps of boots 7 through 9 are the terminal state of a
*completed* tour. The parked receivers are the UART demo's driver and its byte receiver, the wake
was the worker's real send. And `svc=20` is the choreography's exact ecall total, each identified
from the tree rather than from the dump. `notes/scheduler.md` states the consequence plainly: "So
the gate has never fired on a field failure, and `refuse:` has never appeared on a board ring."

The `wake_load_aware` gate and the pop-own-current guard stay. And they earn their keep on their own
merits: the transition they forbid really would complete a rendezvous off a stale mailbox, they are
proven red-then-green by injection tests that drive the real wake path. And
`crates/thread_wake_handshake` models the protocol under loom. What is gone is the field failure
they were built against.

So the honest statement of this risk's evidence is that it has never fired, and the correction
propagated badly. This file was written on 2026-08-30, milestone 201 (is multicore reliability
converging) was minted on 2026-08-31. And milestone 225 (run the soak on radon, argon and xenon) on
2026-09-02. All three repeat the retracted reading, and 201 additionally lists it as the first of
the "first three data points" its curve is to be seeded from. Those three data points need
re-deriving before that milestone starts: the first is retracted. And the other two are `ap_boot`'s
open bugs in milestone 161 (the x86_64 kernel port), which is `BUILT` since 2026-09-19 and with
milestone 316 (making `NIFE_SMP=2` mean something on x86_64) having fixed the boot-core-identity
defect at its root.

### Every multicore defect this project has found was found without silicon

That cuts against the claim's second clause, and it is the strongest evidence this entry has:

- A release fence with no partner in the clock page's seqlock, found by loom on 2026-08-04, in
  milestone 80 (the hand-rolled atomic protocols, model-checked). A reader could revalidate and
  return a state from one publish beside an offset from another. Milestone 116 (the fences with no
  partner) is the sweep it triggered. And the same mistake was found the same day by a hand audit
  that shared nothing with the harness.
- A double free on the untyped region claim, found by a riscv64 QEMU flake: one panic in 45 loaded
  full-suite runs, in milestone 62 (tests that assert on time)'s acceptance run, fixed 2026-08-18
  ([`notes/object-revocation.md`](../../notes/object-revocation.md)). Loom came afterwards:
  milestone 135 (the region claim, under loom) gates the fix with a falsification witness that
  passes only when the pre-fix protocol still double-frees.
- A boot-core-identity defect that failed `every_secondary_runs_scheduled_work` about half the time
  at two cores on x86_64, found and fixed under QEMU by milestone 316 (which core booted).
- A port revocation that did not reach every core. Milestone 313 (the security audit that was due
  since August) raised it first, as its finding 4 on 2026-09-17. It was then reproduced under QEMU
  and fixed on 2026-09-23 by milestone 315 (a port revoke that reaches every core).
  `PortRange::REVOKE` cleared the capability under `IPC_TABLES` but reset the TSS I/O bitmap on the revoker's core only, leaving a window until
  every other core's next switch. Diagnosed from evidence rather than argument: a snapshot at the
  revoke read "revoker on cpu 0, cpu 1 holds a grant" on both captured failures, while 27 passing
  runs had no grant installed elsewhere. The fix resets this core and rides the TLB shootdown's NMI
  to the rest. And moves `install_port_grant` inside the locked region so a core cannot reinstall a
  grant the sweep just cleared. Proved by 12 of 12 full two-core suites green, 36 boots. Shown under
  QEMU, not on silicon, which is the fourth such case and bears on this risk's premise below.
  *(Corrected 2026-09-25 under §216 (fatal-risk facts are correctable, and verdicts are the architect's): this said "found the same way", meaning under QEMU. The audit
  found it first and accepted the window on reasoning; QEMU showed it red afterwards, 7 of 12 two-core
  runs. Sources: milestone 315's block and `notes/multicore-defect-curve.md` row D12.)*
- A test that hung in two of three full HVF runs and passes in every TCG run and when run alone
  (`notes/hvf-leg.md`), un-diagnosed.

None of these needed a board. That does not refute the claim, because a defect emulation can find is
not evidence about the class it cannot. But it does retire the version of this entry that treated
silicon as the only productive instrument. The instruments that have actually produced multicore
defects here are loom and a two-core QEMU, and both are cheap.

### What the emulator cannot show, stated precisely rather than as a worry

- TCG's memory model is far stronger than real silicon's. Guest accesses execute in the host's
  program order and MTTCG serializes cross-vCPU visibility through host atomics
  (`notes/visionfive2.md` says so before the runs rather than after). So an acquire that should have
  been an acquire-release passes `script/test`, `script/cpu-matrix` and every CI leg.
- Loom models C11, not ARM and not RISC-V, which `script/interleaving-check`'s own header states. It
  narrows the gap rather than closing it, over five extracted protocols, not the kernel.
- The one real-silicon leg is aarch64 only and never runs in hosted CI. `script/ci-build`'s `hvf`
  row runs the kernel suite at guest EL1 on four physical Apple cores, which is milestone 81 (an HVF
  leg: the test suite on the physical core). And GitHub's macOS arm64 runners are themselves virtual
  machines with no nested virtualization. So it exists on one laptop. It samples real orderings; it
  does not search them.
- riscv64 and x86_64 have no real-silicon leg at all. For those two architectures the whole of this
  risk is reachable only on a bench boot.
- x86_64 had no multicore coverage by default anywhere. And that changed on 2026-09-23 when
  milestone 315 (a port revoke that reaches every core) landed and `NIFE_SMP` defaulted to 2, per
  DECISIONS §153 (how a two-core x86_64 test earns its place). `script/soak-test --arch x86_64` and
  its `remote=0` report want re-checking against that default; this entry will be wrong again if
  they were not updated with it.

So the reachable fraction of this risk under CI is: five hand-extracted protocols under a C11 model,
plus whatever a round-robin TCG interleaving happens to expose on two or four emulated cores. That
is not nothing, and it is not the class the claim names.

### The two load-sensitive reds of 2026-09-22, and which way they cut

`live_swap_tests` (pull request #1101) and `current_cpu_tests` (#1120) were both chased to test
defects rather than kernel defects: each asserted on `memory::free_page_frames()`, a count of every
free frame in the machine, which any neighboring test's teardown can move.
`notes/load-sensitive-assertions.md` has both, and its own diagnostic sorted them correctly before
either was opened, on direction alone: a slow machine produces a deficit, never a surplus. So
"returned 277 of 224 pages" was never a timeout.

It cuts in the suite's favor in one specific way and against it in another, and both are worth
saying. In its favor: each was closed by *narrowing* the assertion rather than by widening a bound,
which is the move milestone 62 (tests that assert on time) forbids by name. And a narrower assertion
is the stronger one, since a global delta of the right size can be reached by the wrong frames
coming back and a scoped one cannot. Against it: a suite whose every load-sensitive red so far has
resolved to a test bug has never once caught the thing this risk names. And that is equally
consistent with a healthy kernel and with an instrument that cannot see. The note itself refuses to
close the second one, in its own words, that this is not explained by load and may be a real bug.
And it was still un-investigated when the fix landed.

And a third reading is the one worth acting on, because it is cheap. The defect class is not two
sites. `memory::free_page_frames()` is read at 46 places in `kernel/src`, across
`kernel/src/user/force_kill_tests.rs`, `kernel/src/user/cpu_time_tests.rs`,
`kernel/src/user/tests.rs` and `kernel/src/testing.rs`. And many of those reads are the same
before-and-after pair that produced both of these reds. One of them already carries a comment naming
the hazard. So the same failure will recur on a different line, and every recurrence costs a lane an
afternoon deciding whether it is the kernel. A sweep of those sites is worth a milestone of its own.
And it is worth more to this risk than it looks: until a load-sensitive red can be trusted to mean
something, this suite cannot be the instrument that this entry's experiment needs.

The decisive experiment: milestone 225 (run the soak on radon, argon and xenon), `PARTIAL` since
radon ran it on 2026-09-25 and gated `HARDWARE` for the other two. Everything it needs exists and none of it did
on 2026-09-01: a workload that lasts, which is milestone 219 (the boot tour ends and the kernel
halts, so there is nothing to soak). a hook that makes it cross cores, which is milestone 221 (the
soak never crosses cores, so build the hook that makes it). a console that watches and judges, which
is milestone 216 (nothing in this tree can read a board). And a boot that needs nobody typing, which
is milestone 218 (every boot of the VisionFive 2 needs a human typing four commands into U-Boot).
`script/soak-test` is its rehearsal under emulation, and it says on every green run that a clean run
is a number and not a verdict. Milestone 201 (is multicore reliability converging) is the reframe
that makes a red result possible at all, and the running order carries it rather than 225.

### What would render a verdict, in the order it should be bought

1. **Done 2026-09-25**, first heartbeat read before calef left. A bench evening on radon with
   milestone 225's procedure followed, which means reading the first
   heartbeat before walking away: `wakerate` about `100 * harts`, and `crossings` rising between
   beats. Eight hours of a non-crossing soak is milestone 219's experiment wearing 221's name, and
   the difference is invisible afterwards.
2. Milestone 315 landed and `NIFE_SMP` defaulted to 2, which is the cheapest item here and the one
   that converts x86_64 from no evidence to some. It is a lane, not a bench evening.
3. Milestone 201's three seed data points re-derived from the record, since one is retracted and two
   have moved. A curve seeded from a retracted defect is worse than an unseeded one.
4. **Done 2026-10-07**, a stated duration. calef ruled in §259 (a multicore soak counts toward
   risk 5 at ten million crossings over three boots): at least 10 million crossings per
   architecture across at least 3 boots, stated before the run. radon's 4.1 million is almost all
   one boot, so it needs about 6 million more over at least 2 more boots; xenon needs the full 10
   million. Defects found do not reset the count.
5. argon, which has never booted nife at all and sits behind milestone 127 (the seL4 machine).

### What none of that would return is a green

201's own framing is the honest one: a flattening defect-discovery curve is a confidence, a linear
one is the red result. And there is no experiment on this list that can come back saying the
concurrency is correct.
