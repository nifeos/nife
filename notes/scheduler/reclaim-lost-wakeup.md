# The lost wakeup on `reclaim_frees_a_started_then_exited_childs_regions` (closed)

*Name: provisional.*

An appendix to [notes/scheduler.md](../scheduler.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). It is the full record of milestone 72 (lost wakeup): the mechanism, the proof, the ISA skew and
the thread accumulation it was not.

A refused region reclaim killed the child the test was waiting for. One line of test code, no
kernel defect, and not a RISC-V defect either: reproduced on aarch64 the moment the window was
widened. Milestone 72, 2026-08-03.

## What it was

The test opened by probing the refusal:

```rust
crate::sched::start_tcb(tid, [0; 3]).expect("start");
// "Ready but not yet run ... The refusal leaves the region untouched."
assert!(crate::sched::reclaim_region(tcb_region).is_err());
let got = crate::sched::ipc_receive(report)[0];   // waits for the child's SEND
```

That comment was true when it was written and stopped being true when DECISIONS §16 was amended.
A refused reclaim is not passive any more: `reap_region_objects` sets `killed = true` on every
live thread in the region and *then* returns `Err`, so the owner's retry can tear a runaway down.
§24's `^C` escalation is built on exactly that. So the probe marked the child `killed`, and
`schedule()` converts a killed thread to a corpse at its next preemption:

```rust
if t.killed && t.state == State::Running { t.state = State::Finished; }
```

From there it is a plain race between the child's nine instructions and its own core's next timer
tick. Win it and the child SENDs, the test passes, and the armed kill is harmless because the child
was about to exit anyway. Lose it and the child is reaped without ever sending, `ipc_receive`
blocks forever, every core falls to idle, and the 60 s heartbeat fires.

Why host load moved it from "never seen locally" to one run in four is not measured here. The
likely mechanism is that a vCPU thread the host deschedules comes back with its guest timer deadline
already past. The tick then lands at the first instruction it executes rather than ten milliseconds of
guest work later. The window is wall clock, not instruction count, and an oversubscribed host is
what turns those into different numbers.

## How it was proved, since a one-in-four race is not evidence

Widen the window instead of waiting for it (the method milestone 71 (thread-start) used on the frame fault).
A call-free three-instruction delay loop in front of `REPORT_STUB`, sized to span several ticks:

```text
riscv64   lui t0, 0x4000 ; addi t0, t0, -1 ; bne t0, x0, -4
aarch64   mov x5, #0x4000000 ; subs x5, x5, #1 ; b.ne -4
```

With the probe in place that hangs the watchdog on the first run and every run, on both ISAs.
With the probe removed the same widened child passes. Temporary prints in the two suspect lines
caught the whole chain in order:

```text
[M72] child tid=0xe00000065 started
[M72] ARM kill tid=0xe00000065 state=Running
[M72] reclaim(tcb_region) -> Err(())
[M72] CONVERT killed->Finished tid=0xe00000065
WATCHDOG: no progress for ~60 s. Every core idle, every thread blocked: a lost-wakeup hang.
```

The forced dump matched the four wild occurrences exactly: 101 threads, 109 endpoints, every
thread `wake_pending=false on_cpu=false`, all four inboxes empty. Same fingerprint, same hang.

## The fix, and what it costs

The probe is deleted. Nothing else changed. The refusal's own behavior is proved by
`user::force_kill_tests::destroy_force_kills_a_runaway_and_reclaims_its_region`, which points the
destructive call at a runaway that is *meant* to die, and that is the only subject it can honestly
be pointed at. `reclaim_region` now carries a `BUGS` section saying so where a caller meets it.

Confirmed under the original recipe: four host burners, the riscv64 leg twenty times, 0 watchdog
hangs. Three of the twenty failed on something else, and all three are the
bounded-yield-under-contention class this file already documents further down
(`a_thread_that_never_yields_is_preempted_anyway`,
`a_blocked_waiter_wakes_with_an_error_when_its_endpoint_is_revoked`, and the sibling
`threads_round_robin`). That test's "thread {i} never ran" was `sched.rs:2709` then; 128d9492a made
it a clock-bounded wait on 2026-08-03, and the assertion is `kernel/src/sched.rs:7686` as of
2026-10-11. They fail in 23 s with a named assertion, not at 60 s with a dump, so the two are never
confusable once you look. A 15% rate for that class under four burners is worth someone's attention
on its own; it is not this.

The local aarch64 rate is not measurable with this recipe, and that is worth knowing before
someone tries. Ten pre-fix aarch64 runs under the same four burners gave 0 hangs. That sounds
like "rarer on aarch64" and is not evidence of anything. Five of the ten died earlier in the boot
on the bounded-yield contention flakes, before the suite ever reached this test. The aarch64 leg is
much more prone to those under burners than riscv64 is (5 in 10 against 3 in 20), so the burners
break the instrument before they exercise it. The aarch64 evidence that counts is the widened-window
control and the wild CI hit, neither of which needs a rate.

The fix is not a rate reduction, which is the thing to check it against. Deleting the probe means
the child is never marked `killed`, so the conversion that reaped it has no input and cannot happen
on any machine at any speed. That distinction matters because the observed *rates* vary wildly and
say nothing about whether the cure works. It was never seen locally at first, one in four under four
burners, and three consecutive failures on one CI pull request (#29, docs-only) on the shared
runners. All three numbers are what the mechanism predicts. The loser of the race is decided
by how much wall clock the guest gets between the child being switched in and its `ecall`. A
two-core shared runner emulating four harts gives it little. A hot rate is evidence the window
is wide there, not evidence of a second bug.

Occurrences:

| When | Where | ISA |
|---|---|---|
| 2026-08-03 | CI, `rva23s64` (PR #20) | riscv64 |
| 2026-08-03 | CI, PR #21, which changed two markdown files and zero lines of code | riscv64 |
| 2026-08-03 | CI, `thead-c906` (PR #23, the frame fix), guard silent | riscv64 |
| 2026-08-03 | local, four host burners, 1 run in 4 | riscv64 |
| 2026-08-03 | local, widened window, first run and every run | aarch64 |
| 2026-08-03 | CI, PR #29, which changed `design/roadmap.md` and nothing else | aarch64 |

Every row is the same test and the same watchdog. The last one arrived in the wild, on the aarch64
runner (`helpers/qemu-runner-aarch64.sh`, `target/aarch64-unknown-none-softfloat`), while this milestone was
being written, and it is the independent confirmation of what the widened-window control had already
shown.

## Why the first four were all riscv64, and why it is not a RISC-V property

The answer is exposure, not the ISA, and it is countable rather than arguable. Per pull request,
CI boots the suite seven times: once on aarch64 and once on riscv64 in `build + test`. Then come five
more riscv64 boots in the `cpu matrix` job, which runs `script/cpu-matrix` over `rv64`,
`sifive-u54`, `rva22s64`, `rva23s64` and `thead-c906` and deliberately does not stop at the first
failure. Six riscv64 rolls of the dice to one aarch64 roll. Four riscv64 sightings before the
first aarch64 one is what a 6:1 exposure ratio produces on its own.

Two explanations offered along the way were wrong, and both are recorded because each is the kind
that sounds right:

- "riscv64 loses the race more often under TCG."  Written in an earlier draft of this section, by
  this milestone. Possible, unmeasured, and unnecessary once the exposure ratio is counted.
- "riscv64 runs first, so it failed first and the aarch64 leg never got there."  Offered while the
  aarch64 hit was being reported, and it is backwards: `xtask test` runs the aarch64 leg first
  and `return false`s on its failure, so a riscv64-only sighting is a run in which aarch64 was given
  its chance and passed. The four riscv64 CI hits came from `cpu matrix`, which has no aarch64 leg to
  order against.

§19 says parity is a gate. The corollary this bug supplies is that a failure appearing on one ISA is
a *claim* about that ISA. The cheapest way to test the claim is to widen the window on the other
one, not to reason about what is arch-specific. Careful reasoning was done here, and it pointed at
RISC-V for four days.

## The accumulation is not this bug, and the aarch64 hit does not make it one

The 101 threads and 109 endpoints were the lead everyone followed first, including this
milestone's brief, and they are a real thing that is not this.

The A/B settles it. Under the widened window the tree hangs with one line of test code present
and passes with it removed, on both ISAs, deterministically. The accumulation is identical in
both arms: same tests before it, same 101 threads, same 109 endpoints. A cause you can leave in
place while the effect disappears is not the cause. There is a mechanism for the thing that
does explain it, traced print by print, which the accumulation never had.

It is worth saying because the aarch64 sighting reads at first like evidence *for* the accumulation.
The leak is shared scheduler state present on both ISAs, so a second ISA failing is what you would
predict if the leak were the cause. It is what you would predict from portable `sched.rs` code
and a race, which is what it turned out to be, and the two predictions are the same. A prediction
both hypotheses make cannot choose between them. The A/B can, and did.

The supporting reasons stand on their own too. The threads are blocked, so they add no scheduling
load and no run-queue depth; 109 endpoints is a fifth of `MAX_ENDPOINTS`. The suite arrives at this
test that way on both ISAs (`notes/riscv-parity-scope.md` measured the table at 87 on each at the
leak police), so it never could have explained an ISA skew either.

It is still worth its own milestone: 101 of `MAX_THREADS = 128` is 79% of a hard `create_tcb`
failure, and the leak police only polices *runnable* leaks, so a blocked leak is invisible to it by
construction. Nothing today would warn before the suite hit the wall.

Update, 2026-08-27: it hit the wall, and the warning now exists. The paragraph above was
written as a prediction; the aarch64 suite reached 128 of 128 with 121-123 `Blocked` and had spawns
refused with no diagnostic, exactly as described. `sched::MAX_THREADS` is 256 now, on a measured
peak of 130 (aarch64), 129 (riscv64) and 57 (x86_64), and that constant's own doc comment carries
the ledger and the four coupled constants that moved with it. Two things closed the gap this
section names:

- The occupancy is reported rather than invisible. `sched::PEAK_THREADS` is a whole-boot
  high-water mark and `kernel::testing` prints it in the closing summary beside the frame ledger
  (`threads: N live at the peak, of M the image allows`). A blocked leak is still invisible to
  `thread_leak_police`, which is a different question and correctly scoped to runnable spinners,
  but nobody has to instrument the kernel to see the table filling any more. It reports and does
  not gate, and `report_thread_peak`'s doc says why: what fills this table is what earlier tests
  deliberately left running, so a gate would fire on every milestone that adds a service.
- The refusal says so. `pipeline_service::start_with`'s second spawn had a bare `?`, which is
  why the wall presented as "the test did not get a shell" rather than as a full thread table.

The honest remainder: 130 of 256 is 51%, so the same growth curve reaches the same place again,
and the answer then should be an account of what the boot keeps alive rather than another
doubling. `notes/frames.md`'s "held" list is where that account would start.

What the raise cost, and why the next one will not. The first attempt failed `script/bench
--check`: `spawn_el0` moved +16.8%, through two whole-table scans whose cost tracked the ceiling
rather than the occupancy (the capability sweep, and `revoke`'s space registry). Both are bounded
by live occupancy now, which left that benchmark 41% below where it had been at 128 slots and, more
to the point, flat against the constant: doubling to 512 moves it 0.05%. So the next raise is
cheap in the one place this one was not. `notes/benchmarks/spawn-el0.md`'s 2026-08-27 section has the
attribution, and `MILESTONE 183` is the structural fix the mitigation does not replace.

## The general lesson

A call that returns `Err` may still have done something. `reclaim_region(r).is_err()` reads like
a question and is an act; the test used it as a question and the comment beside it asserted the
opposite of the truth. When a semantic is amended, the amendment lands in the function it changed,
and every caller that encoded the old semantic in a *comment* keeps compiling.
