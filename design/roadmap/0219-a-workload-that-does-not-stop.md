---
status: BUILT
raised: 2026-09-01
built: 2026-09-01
---
# 219. The boot tour ends and the kernel halts, so there is nothing to soak

Built (2026-09-01). Minted the same day by the maintainer, after radon booted under
script control and the gap became obvious: everything needed for a sustained run exists except a
workload that lasts. *(Number provisional until the merge queue lands it.)*

It needed no board to build and no board to test (this block carried `Gate: NONE`, now removed
because a finished milestone gates nothing). QEMU runs it; a board is only where the answer becomes
interesting, and no board has run it yet.

In brief. `design/fatal-risks/README.md` risk 5 (it cannot be made reliable on multicore, and the bugs
appear only on silicon) names its decisive experiment as *sustained multi-core stress on the boards
with the load-sensitive assertions live*. At the time this milestone was minted (2026-09-01) the risk
was believed to have already fired once, on radon, with a receiver woken and nothing delivered on
three harts, found in a bench session rather than by any test. That reading is retracted
(`notes/visionfive2.md`'s fifth bench stop, 2026-08-15, before this milestone was minted): the dumps
were the terminal state of a completed boot tour, not a stranded receiver, and `notes/scheduler.md` ("The undelivered-wake gate")
records that the gate built against it "has never fired on a field failure." Found still repeating the
retracted reading here 2026-09-23. The rest of this milestone (a workload that lasts, so there is
something to soak at all) does not depend on the retracted finding and stands regardless.

Nothing in this tree could sustain anything. The kernel's boot tour ran its checks, printed
`nife: the capability core runs on RISC-V.`, and called `halt()`. Captured on radon on 2026-09-01,
that is the last line, after which the board sits in `wfi` indefinitely. A soak needs a workload
that keeps running, and there was not one.

That was the whole milestone: something that runs on every core, for hours, that fails loudly.

## What was built

`--features soak` replaces the halt at the end of the boot tour with a pool of user-mode workers and
a supervisor that watches them forever. `notes/soak.md` is the account; the pieces are
`kernel/src/soak.rs`, `fixtures/src/soaker.rs`, `crates/soak_page`, `script/soak`, `script/board-image
--soak`, and `Stage::Soak` in `crates/serial_console`.

It runs on all three architectures under QEMU and is the same workload that would run on radon,
argon and xenon. `script/soak --arch <a>` boots it and judges it with the same recognizer and the
same policy `script/board-console` points at a board, so the rehearsal and the experiment are one
thing with different deadlines.

### The four questions this block left open, answered with evidence

- What it stresses: cross-core IPC rendezvous, at the highest rate the machine will do them. Not
  a guess about where the bugs are, but where the one bug this risk produced was. A round trip is
  `CALL` -> `RECV_CAP` -> `REPLY` -> the caller waking: two block/wake handshakes, the protocol
  `crates/thread_wake_handshake` models. Each worker jitters between round trips so the pairs'
  phase keeps drifting; a soak that repeats one interleaving for eight hours has explored one
  interleaving.
- How it fails loudly: three verdicts, checked every beat, each ending in a thread dump and a
  panic. A refused wake (the gate in `sched::wake_load_aware` firing, which is the defect itself),
  a caller getting back a word that is not the answer to what it sent, or a worker making no
  progress for a whole beat.

  **Which existing instrument it extends, and why the other two could not be it.** It extends
  `crates/serial_console`, so the QEMU rehearsal and the bench run are judged by one recognizer.
  `script/repeat-under-load` repeats a terminating suite under induced host load and reports a
  distribution over runs; a soak has no runs to repeat, and the load it wants is the guest's own.
  `script/interleaving-check` is loom over the extracted protocols on the host, which searches a
  state space rather than watching a machine, and is the complementary evidence rather than the
  same evidence. Both are named in `notes/soak.md` as what they are: neither was the right place to
  put a boot that never ends.
- A user program, with the detection in the kernel. The defect is causable from userspace
  through the real syscall path, so a kernel-mode stress loop would be testing an artifact of the
  test. A user program cannot assert about kernel internals, so the assertions stay where the
  trace counters are. The two meet over one shared page with one writer per word
  (`crates/soak_page`).
- What "it passed" means: a round-trip total, and three comparisons it supports. Between
  architectures, between QEMU and silicon, and against the same machine later. First numbers, QEMU
  on patagonia, 2026-09-01: aarch64 ~58,000/s on four cores, riscv64 ~24,000/s on four,
  x86_64 ~3,900/s on one. All three from a `--features soak` build, whose IPC path is 1.05 to
  1.06x the production one; compare a soak number only with another soak number. Nothing else. See
  the BUGS section, which is the important half.

### And a fifth question this block did not think to ask

**A saturated workload does not migrate between cores under this scheduler.** Measured across three
topologies and both multicore architectures: the cross-core handoff count freezes within the first
second and never moves again, while the machine does tens of thousands of round trips a second.
Rendezvous wakes are local by design (DECISIONS §28.2), `wake_load_aware` is reachable only from a
device interrupt, a work steal needs an idle core *and* a queued thread elsewhere, and nothing
rebalances periodically.

So "sustained multi-core stress" is **two** experiments, and this milestone delivers one of them:
concurrent contention on shared kernel state, at rate, on every core. It does not deliver cross-core
handoff under load, which is where the observed defect lived. `script/soak` prints that gap on every
run rather than leaving it to be discovered.

The instrument that found this was the second one. The first counted `trace::Event::PlaceRemote`,
which is structurally blind to the migration a rendezvous performs, because the wake queues the peer
on the waker's own core and so the *placement* is local even when the *thread* has moved.
`thread::Thread::last_cpu` and `trace::Event::Migrated` answer the question at `schedule()`'s
`switch_in`, which every path to a CPU passes through.

## How a hang is distinguished from a slow run

**The heartbeat is on the wall clock, not on the work.** A machine doing one round trip a second
still prints on time, with a rate that says so; a machine doing none still prints, and its stall
check fires. Silence means the thing that prints is itself wedged, which is the only thing silence
is allowed to mean.

`crates/serial_console` implements the other half rather than agreeing to it in prose. `Stage::Soak`
is reached by the kernel's own `soak: started` line, and reaching it re-arms the quiet check that
a completed boot tour suppresses: a halted kernel is supposed to be quiet and a soaking one is not.
One word (`< Stage::Tour` became `!= Stage::Tour`), asserted on a real QEMU capture cut off after its
second heartbeat. The beat is five seconds against a fifteen-second default quiet window: three
missed beats, exit status 2.

## What would make it a real answer to risk 5

Running it on **radon**, **argon** and xenon, not just in QEMU, since the entire premise of the
risk is that emulation cannot show these defects. `notes/soak.md` carries the bench procedure, step
by step, including which flag builds the payload in the order the measured-boot gate requires.

None of the three has been run at a bench yet. That is the remaining half of this milestone and
it needs a person, a board and an evening, not a lane.

## Why the instrumentation is behind a feature

Built unconditionally, the instrumentation grows `ipc_fastpath` 5.7% on aarch64 (5,788 -> 6,120
bytes), over the 5% bound of milestone 132 (the fast path's footprint), with riscv64 and x86_64 at
4.7% and 4.6%, and `script/fastpath-footprint` refuses it. One cause with three effects; aarch64 was
merely the one that tipped. The single largest contributor is the `Thread::last_cpu` write, which
sits in `schedule()`'s switch, the hottest line of the hottest function, and the per-event counter
increments are the rest.

So the counters, the accessors and the `last_cpu` field are `#[cfg(feature = "soak")]`, and a
production build is byte-identical without them: measured at 5,852 / 5,132 / 6,687, the same three
numbers as the build with no instrumentation. The 5% bound was not touched, and it should not be: it
is Liedtke's cache-footprint argument, which is the oldest performance case in microkernels and the
reason the gate exists.

Two consequences worth stating rather than leaving to be found. **A soak build is not a production
build**, its IPC path is five to six per cent larger, and the round-trip figures above are therefore
soak-build figures; `notes/soak.md` carries the per-architecture table. And a `cfg`-gated instrument
is invisible to clippy, so `script/lint`'s per-feature loop includes `soak` on both ISAs, which
on its first run found two real warnings in code nothing had ever linted.

## BUGS

- This block sets no duration, and the honest reason is that nobody knows what duration would
  be persuasive. The risk's own text says this class "produces a confidence rather than a verdict".
- A soak that finds nothing is weak evidence and must be reported as such. The failure mode to
  guard against is a green run being quoted as though it proved the concurrency correct. What a
  clean eight hours licenses is one sentence: *this machine did N cross-core IPC round trips without
  the wake gate refusing one, without a wrong reply, and without a worker stalling.* `script/soak`
  prints that caveat on every green run, because a number quoted without it is quoted wrongly.
- **The soak does not cover the path the observed defect was on.** `wake_load_aware` takes a device
  interrupt to reach, and no user workload can raise one. This is the sharpest form of the caveat
  above and it was not known when this block was written.
- The heartbeat is guest time and the watcher's deadline is host time, so a QEMU guest on a
  heavily loaded host can produce a false `WentQuiet`. `--quiet-after` is the knob; not running a
  soak beside other heavy work is the better answer.
- **A soak build is not the binary that ships.** Its `ipc_fastpath` is 1.05 to 1.06x the production
  one, so its timing differs, and nothing in this milestone's numbers is a statement about how fast
  this kernel does IPC. `script/bench` is the instrument for that.
- Nothing runs a soak inside `script/test`, so the feature can stop compiling without anything
  saying so until someone runs `script/soak`. A twenty-second leg per architecture would close it
  and was judged too expensive for a gate every lane runs.
- `--arch x86_64` soaks one core unless told otherwise, because that runner defaults to one and
  its SMP bring-up has two open bugs (`arch::x86_64::ap_boot`'s BUGS #1 and #3). Its `crossings=0`
  says so out loud, and a single-core soak is not a multicore soak.

## Handoff

Two proposed milestones, both of which this lane found and neither of which it should decide,
because each is a scheduler-policy or syscall-surface question and those are an architect's:

- Proposed milestone: a workload can be made to cross cores, or it is admitted that none can.
  The options are thread affinity (pin a responder so its callers must reach it from elsewhere), a
  periodic rebalancer, or a userspace-drivable interrupt source. Each changes what the scheduler
  promises; the first two also change what a program may ask for. Until one exists, the second half
  of risk 5's decisive experiment cannot be run at all, which is a fact about the risk rather than
  about this workload.
- Proposed milestone: run the soak on radon, argon and xenon. The procedure is written and the
  tooling is built; what it needs is a bench, a night, and the numbers recorded in
  `notes/soak.md`'s table beside the QEMU rows. Milestone 218 (every boot needs a human typing four
  commands into U-Boot) makes it cheaper but does not block it.

## Follow-on

- **Milestone 221.** The first proposed milestone in this block's handoff: a workload can be made to
  cross cores, or it is admitted that none can. calef answered the fork in
  `design/decisions/0138-cross-core-handoff-under-load.md` and 221 built the `sched::on_tick` hook.
- **Milestone 225.** The second: run the soak on radon, argon and xenon, which is where its answer
  means anything. The procedure is written and the tooling is built; it needs a bench and an evening.
- **Milestone 245.** The duration this block declined to set. Every counter `script/soak-test` prints is
  a volume, so no beat can be compared with the one before it and a run that stopped learning looks
  exactly like one that has not.
- **Refused.** A soak leg inside `script/test`. Twenty seconds per architecture would stop the
  feature silently ceasing to compile, and it was judged too expensive for a gate every lane runs on
  every push.
  This refusal is milestone 473 (design/roadmap/0473-a-soak-leg-in-script-test.md), which carries it
  with the condition that would change it.
- **Recorded.** `notes/soak.md`: a soak that finds nothing is weak evidence, and what a clean run
  licenses is one sentence about round trips completed without a refused wake, a wrong reply or a
  stalled worker. `script/soak-test` prints that caveat on every green run.
- **Recorded.** `notes/soak.md`: a soak build is not the binary that ships. Its IPC fast path is
  1.05 to 1.06x the production one, so no number here is a statement about how fast this kernel does
  IPC, and `script/bench` is the instrument for that.
- **Recorded.** `design/roadmap/0219-a-workload-that-does-not-stop.md`: the heartbeat is guest time
  and the watcher's deadline is host time, so a QEMU guest on a loaded host can produce a false
  quiet verdict. The knob is `--quiet-after`, and not soaking beside other heavy work is the better
  answer.
- **Recorded.** `design/roadmap/0219-a-workload-that-does-not-stop.md`: an x86_64 soak runs one core
  unless told otherwise, because that runner defaults to one and its SMP bring-up has two open bugs.
  A crossing count of zero says so out loud, and a single-core soak is not a multicore soak.

## Index row

`--features soak` replaces the halt at the end of the boot tour with user-mode IPC workers and a
supervisor that beats every five seconds, on all three architectures. The workload is a user
program and the detection is in the kernel, because the one defect risk 5 produced is causable
from userspace and assertable only from inside. `script/soak` judges the QEMU run with the same
recognizer `script/board-console` points at a board, and `Stage::Soak` re-arms the quiet check a
completed tour suppresses, so a hang and a slow run are told apart by one rule both halves
implement. First numbers: aarch64 ~58,000 round trips/s on four cores, riscv64 ~24,000, x86_64
~3,900 on one. **And a finding the block did not think to ask for: a saturated workload does not
migrate between cores under this scheduler**, measured across three topologies and both multicore
architectures. The soak sustains contention on shared kernel state and cannot sustain
cross-core handoff, which is where the observed defect lived. No board has been run yet.
