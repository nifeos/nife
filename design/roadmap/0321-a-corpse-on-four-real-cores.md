---
status: PARTIAL
raised: 2026-09-17
milestone_dependencies: none
decision_dependencies: none
machine_requirements: four-core x86_64 silicon
specific_machine: xenon (the corpse failure was seen there; rerun the x86_64 suite at NIFE_SMP=4)
needs_person: yes
---
# 321. A corpse that did not park, on four real cores

The transcript separates all three readings, and the failure does not reproduce on anything
patagonia can run. Reproducing it needs four-core x86_64 silicon: see "What the reproduction
attempt established" below. The investigating half is done and is what this block delivered.

This is the one failure from xenon's 2026-09-17 bench evening that is not an assertion written
against QEMU. The other three were: a q35 chipset id, an eight-yield window, and a one-bus PCI
scan. This one is about what the kernel did.

## What happened

```
test kernel::user::survey_tests::a_dead_child_is_still_in_the_domain_until_it_is_reaped ...
  user thread 17179869238 killed: vector 14 (page fault)
    rip 0x0000000000400005   addr 0x0000000000a50000   user rsp 0x0000000000501000   err 0x00000004
  the kernel is fine.

[PANIC] panicked at kernel/src/user/survey_tests.rs:381:5:
```

The child died. Its kill message is in the transcript three lines above the panic, at the fault
address `FAULT_STUB` is written to produce. What did not happen is the corpse parking on its
supervision rendezvous's sender queue, where a survey can see it before anyone has collected it.
That is DECISIONS §26's property and `capability::survey_includes`' proved one.

Transcript: `bench/xenon-2026-09-17/nvme-attempt-2-no-controller-found.log`.

## Do not widen the bound

The reflex, after an evening in which two other failures were margins real silicon closed, is to
read this the same way. `wait_for`'s bound is already two seconds of wall clock:

```rust
let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
```

It yields until then and checks once more on the way out. Two seconds is not a thin margin on a
2.7 GHz machine for a thread already reported dead. A test made to pass by waiting longer, for
something that should already have happened, has been silenced rather than fixed.

The bound was not widened, and it is now positively ruled out rather than merely left alone.
Two pieces of evidence, both from the transcript itself:

- The boot print reads `clocks : tsc 2714 MHz (PIT calibration)`, so `frequency()` returned a sane
  number and `2 * frequency()` really was about two seconds.
- 61 call sites use `wait_for` across the kernel's user tests, and this run failed exactly
  one of them. A `wait_for` whose deadline arithmetic was wrong on this machine would have taken
  a great many of the other sixty with it.

So the two seconds elapsed, the predicate was sampled throughout, and it was never true.

## Three readings, and the transcript separated none of them

1. The corpse parked and something took it off again. Then the survey cannot see it either, and
   §26's property is false on this configuration.
2. The corpse never parked, because death delivery on a core other than the one running the
   test took a path that does not enqueue the sender.
3. The count was not 1. The assertion is `== 1`, not `>= 1`, so a second sender on that queue
   fails it in a way that reads identically in the log.

The next transcript will separate all three, which is what this block built. The assertion now
carries the count *and* `sched::thread_death_disposition(child)`, which reports the corpse's run
state, its `fault_ep` and its recorded wait. Those three fields differ in every history:

| What the message shows | What happened |
|---|---|
| `Finished`, `fault_ep` `None` | `depart` took the **unsupervised** path: supervision was lost before the fault, not in delivery |
| `Dead`, wait `Some((ep, Sender))` | The corpse **did** park and something removed it: reading 1 |
| `Dead`, wait `None` | `deliver_death`'s `send` met a waiting receiver, so the corpse never joined the queue: reading 2 |
| `Ready`, `Running` or `Blocked` | It never departed, whatever was printed about it |
| `None` | The thread was reaped and freed |

The count alone would have settled only reading 3, which is why the disposition is there too: a
count of zero leaves readings 1 and 2 still identical in a log. The message was verified by
forcing the assertion to fail, rather than shipped unseen, and printed

```
the child never died onto its supervision rendezvous: 1 senders parked on it,
and the child is Some((Dead, Some(17179869184), Some((17179869184, Sender))))
```

which is the healthy shape. That check earned its keep: on xenon the panic's own message never
reached the console at all. `kernel/src/panic.rs` prints `[PANIC] {info}` as one statement whose
`Display` spans two lines, and the log stops between them, so a diagnostic that had never been seen
render would have been worth nothing.

## What the reproduction attempt established

It did not reproduce on anything patagonia can run, and the shape of that failure is the
finding:

- x86_64 under QEMU at `NIFE_SMP=4`: one full suite plus five filtered runs, all green, with
  `smp: 4 core(s) online` confirmed in the boot print (so this was not `ap_boot`'s BUGS #1 quietly
  running three).
- x86_64 under QEMU at `NIFE_SMP=8`: ten filtered runs, all green.
- x86_64 under QEMU at `NIFE_SMP=4` with genuinely parallel cores: twelve filtered runs, all
  green. See below; this leg did not exist before this milestone.
- aarch64 at four cores: `helpers/qemu-runner-aarch64.sh` defaults `NIFE_SMP` to 4, so every
  ordinary `script/test` has been running this test at four cores all along, and it passes.

### The missing condition was parallelism, not cores, and that is now measured

TCG has two vCPU models, and this port had only ever used one. Round-robin (`thread=single`) runs
every guest core on one host thread; multi-threaded TCG (`thread=multi`) gives each guest core
its own. On an aarch64 host running an x86_64 guest QEMU picks round-robin, so `NIFE_SMP=4` in
this tree has never meant four cores running in the same instant. Measured rather than asserted: at
`-smp 4`, QEMU 11.1.1 creates 5 threads under `thread=single` and 8 under `thread=multi`.

So the first sixteen runs above were not the experiment they looked like. `NIFE_TCG_THREAD`
(provisional name) was added to `helpers/qemu-runner-x86_64.sh` to fix that, is empty by default
so nothing else changes. Its `BUGS` section carries the caveat that matters: x86's TSO is
stronger than an aarch64 host provides, QEMU accepts the pair without a warning, and nobody here has
audited its barrier placement. A red run under that knob would need confirming on a bench before
it was believed. Twelve green ones need no such care, and twelve is what it produced.

The other path to parallel cores, the aarch64 HVF leg, does not run here: QEMU refuses
`virt,accel=hvf,gic-version=2,iommu=smmuv3`, the known GICv2/GICv3 incompatibility that milestones
227 and 317 own and that `script/ci-build` already skips out loud. That is somebody else's bug and
was not worked around.

So the honest statement is the one `design/fatal-risks/README.md` risk 2 predicts: this is reachable only
on real silicon, or it is rarer than 28 runs. The instrument for reading it when it next
happens is now in the tree rather than on a bench.

### A second failure under emulation, past the assertion this block instruments

Two hundred runs per boot (`--repeat`) found one at four interleaved cores under OVMF, the
firmware leg `cargo xtask uefi-test` boots, on 2026-10-21 at about 23:57 UTC:

```
repeat result: FAILED. 52 runs of 200, 51 passed, first failure at run 52; its message is below

[PANIC] panicked at system_tests/src/user/survey_tests.rs:410:5:
assertion `left == right` failed: the tid a survey reported was not one the same rendezvous could reap
  left: Err(NotPermitted)
 right: Ok(0)
```

The corpse parked and the survey listed it `DEAD`, so this block's assertion held. `REAP` then
passed `reap_decision` (supervised and `Dead`), and `reclaim_region` refused, which
`reap_supervised` reports as `NotPermitted`. Its comment names two causes, a child region and a
racing reap. Neither is established. A third is not ruled out: `Dead` visible while the dying
thread is still leaving its core. That is again the cross-core death this block points at.

It was one failure in 255 OVMF runs. The PVH runner had none in 403 interleaved runs, nor in 200
with `NIFE_TCG_THREAD=multi`.

## What reading the code ruled out, and what it did not

Reading is not measurement and this section is careful to say which is which. Three of the
candidate mechanisms have no path in the source and are ruled out by inspection:

- **The unsupervised path.** `build_child_in` inserts the fault-endpoint capability into
  `FAULT_EP_SLOT` before `start_thread_control_block`, and that function records `t.fault_ep`
  and consumes the slot under `IPC_TABLES` before the thread is ever `Ready`. There is no window in
  which a startable child has no supervision endpoint.
- **A recycled rendezvous carrying stale queue entries.** `try_create_rendezvous_from` retypes a
  fresh page and explicitly writes `Rendezvous::new()` over it, and `rendezvous_table` is
  generational. A new `ep` cannot inherit an old one's parked sender or, worse, an old one's parked
  *receiver*, which would swallow the death message through `Send::Rendezvous` and leave the count
  at zero forever.
- **Something dequeuing the corpse.** The only things that remove a sender are `Rendezvous::receive`,
  `remove_sender` and teardown. This test reaches none of them before the assertion.

**What that leaves is reading 2 by a route nobody has yet named**, and this block does not claim to
know it. `start_thread_control_block` places a fresh child on another core's inbox
(`place_on(pick_spawn_target(), ptr)`) and sends a reschedule IPI; the death then happens on that
core, under a lock this one also takes. That is the region where a genuinely parallel machine can
differ from an interleaved one, and it is where the next bench run's disposition line will point.

**One thing deliberately not concluded:** that the kernel is correct. Twenty-eight green runs, twelve
of them with really parallel cores, are not evidence that the condition is harmless on a machine
none of them reproduced. "It passed on a retry" is what this block refuses, and "it passed
twenty-eight times somewhere else" is the same refusal wearing a bigger number.

## Why it is worth a lane rather than a retry

This is the supervision mechanism. A corpse a supervisor cannot see is the failure §26 (the fault endpoint) exists to
prevent, and the test's own doc comment says why both halves live in one case. The domain the
viewer reports and the domain the supervisor may collect from are one set. If that set differs on a
multi-core machine, a property this tree proves and tests holds under emulation and is unverified
where it matters.

**x86_64 running four real cores is new.** §153 flipped `NIFE_SMP` to 2 for the runner and this
machine boots 4, and real cores at real speed is the condition under which this tree has
historically found its scheduler bugs. `design/fatal-risks/README.md` risk 2 cites the VisionFive 2's
undelivered-wake bug for exactly this: found on a bench, invisible in QEMU, and no proof was
positioned to see it.

## Done when

Either a defect is found and fixed with a test that fails without the fix, or the investigation
establishes that the kernel is correct and the test was wrong, with the reason written down.

"It passed on a retry" is not an outcome this block accepts.

**Neither has happened yet, which is why this is `PARTIAL` and not `BUILT`.** What has happened is
that the next occurrence answers the question by itself, from the log, without the bench being
available again.

## Index row

A child died, the kernel reported it dead, and its corpse never reached its supervision rendezvous
within two seconds. DECISIONS §26's property, on four real cores, and the one failure from xenon's
bench evening that was not an assertion written against QEMU.

## Follow-on

- **Outstanding.** The failure itself is unresolved and is gated on xenon: run the x86_64 suite at
  `NIFE_SMP=4` on that machine. If the assertion fires, its message now names which of the three
  readings is true. If it does not fire, the frequency of the original is itself worth recording,
  because a one-in-many failure in supervision is not a lesser finding than a deterministic one.
  One boot measures it: `cargo xtask uefi-test --test
  a_dead_child_is_still_in_the_domain_until_it_is_reaped --repeat 200 --stage-only` stages a
  stick image that runs the test 200 times and stops at the first failure, naming its run
  (`notes/scripts.md`, "Repeating it").
- **Milestone 315.** `kernel::user::x86_port_tests::a_revoked_holder_faults_on_its_next_port_write`
  also fails at `NIFE_SMP=4`, and this lane spent a while treating that as its own find before
  reading the runner it had been editing. `helpers/qemu-runner-x86_64.sh`'s own header already names
  it and root-causes it (`PortRange::REVOKE` resets the TSS I/O bitmap on the revoker's core only, so a
  holder on another core keeps the ports for up to a tick). It points at
  [milestone 315](0315-port-revoke-every-core.md), which closes it and is `NOT-STARTED` with no gate.
  Nothing is owed here, and the near-miss is worth the sentence: a proposal file had been written
  for it before the existing record turned up.
- **Done.** `NIFE_TCG_THREAD=multi` in `helpers/qemu-runner-x86_64.sh` gives this port parallel
  cores under emulation for the first time. Provisional name, empty by default, and its `BUGS`
  section is honest that it is a hunting instrument rather than a gate. Whether the suite should
  ever run that way is not this lane's call and is left alone; §153 and milestone 315 are already
  holding the x86 SMP posture.
- **Recorded.** `kernel/src/arch/x86_64/timer.rs`'s `BUGS` now says that `now()` reads the calling
  core's TSC while the other two ports read a system counter, and that nothing measures or corrects
  the skew. Investigated as the explanation for this failure and ruled out; recorded because the
  asymmetry is real and was undocumented.
